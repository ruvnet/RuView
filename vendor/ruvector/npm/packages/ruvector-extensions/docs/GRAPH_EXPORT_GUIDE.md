# Graph Export Module - Complete Guide

## Overview

The Graph Export module provides powerful tools for exporting vector similarity graphs to multiple formats for visualization, analysis, and graph database integration.

## Supported Formats

| Format | Description | Use Cases |
|--------|-------------|-----------|
| **GraphML** | XML-based graph format | Gephi, yEd, NetworkX, igraph, Cytoscape |
| **GEXF** | Graph Exchange XML Format | Gephi visualization (recommended) |
| **Neo4j** | Cypher queries | Graph database import and queries |
| **D3.js** | JSON for web visualization | Interactive web-based force graphs |
| **NetworkX** | Python graph library format | Network analysis in Python |

## Quick Examples

### 1. Basic Export to All Formats

```typescript
import { buildGraphFromEntries, exportGraph } from 'ruvector-extensions';

const entries = [
  { id: 'doc1', vector: [0.1, 0.2, 0.3], metadata: { title: 'AI' } },
  { id: 'doc2', vector: [0.15, 0.25, 0.35], metadata: { title: 'ML' } },
  { id: 'doc3', vector: [0.8, 0.1, 0.05], metadata: { title: 'History' } }
];

const graph = buildGraphFromEntries(entries, {
  maxNeighbors: 5,
  threshold: 0.7
});

// Export to different formats
const graphml = exportGraph(graph, 'graphml');
const gexf = exportGraph(graph, 'gexf');
const neo4j = exportGraph(graph, 'neo4j');
const d3 = exportGraph(graph, 'd3');
const networkx = exportGraph(graph, 'networkx');
```

### 2. GraphML Export for Gephi

```typescript
import { exportToGraphML } from 'ruvector-extensions';
import { writeFile } from 'fs/promises';

const graphml = exportToGraphML(graph, {
  graphName: 'Document Similarity Network',
  includeMetadata: true,
  includeVectors: false
});

await writeFile('network.graphml', graphml);
```

**Import into Gephi:**
1. Open Gephi
2. File → Open → Select `network.graphml`
3. Choose "Undirected" or "Directed" graph
4. Apply layout (ForceAtlas2 recommended)
5. Analyze with built-in metrics

### 3. GEXF Export for Advanced Gephi Features

```typescript
import { exportToGEXF } from 'ruvector-extensions';

const gexf = exportToGEXF(graph, {
  graphName: 'Knowledge Graph',
  graphDescription: 'Vector embeddings similarity network',
  includeMetadata: true
});

await writeFile('network.gexf', gexf);
```

**Gephi Workflow:**
- Import the GEXF file
- Use Statistics panel for centrality measures
- Apply community detection (Modularity)
- Color nodes by cluster
- Size nodes by degree centrality
- Export as PNG/SVG for publications

### 4. Neo4j Graph Database

```typescript
import { exportToNeo4j } from 'ruvector-extensions';

const cypher = exportToNeo4j(graph, {
  includeVectors: true,
  includeMetadata: true
});

await writeFile('import.cypher', cypher);
```

**Import into Neo4j:**

```bash
# Option 1: Neo4j Browser
# Copy and paste the Cypher queries

# Option 2: cypher-shell
cypher-shell -f import.cypher

# Option 3: Node.js driver
import neo4j from 'neo4j-driver';

const driver = neo4j.driver('bolt://localhost:7687');
const session = driver.session();

await session.run(cypher);
```

**Query Examples:**

```cypher
// Find most similar vectors
MATCH (v:Vector)-[r:SIMILAR_TO]->(other:Vector)
WHERE v.id = 'doc1'
RETURN other.label, r.weight
ORDER BY r.weight DESC
LIMIT 5;

// Find communities
CALL gds.louvain.stream('myGraph')
YIELD nodeId, communityId
RETURN gds.util.asNode(nodeId).label AS node, communityId;

// Path finding
MATCH path = shortestPath(
  (a:Vector {id: 'doc1'})-[*]-(b:Vector {id: 'doc10'})
)
RETURN path;
```

### 5. D3.js Web Visualization

```typescript
import { exportToD3 } from 'ruvector-extensions';

const d3Data = exportToD3(graph, {
  includeMetadata: true
});

// Save for web app
await writeFile('public/graph-data.json', JSON.stringify(d3Data));
```

**HTML Visualization:**

```html
<!DOCTYPE html>
<html>
<head>
  <script src="https://d3js.org/d3.v7.min.js"></script>
  <style>
    .links line { stroke: #999; stroke-opacity: 0.6; }
    .nodes circle { stroke: #fff; stroke-width: 1.5px; }
  </style>
</head>
<body>
  <svg width="960" height="600"></svg>
  <script>
    d3.json('graph-data.json').then(data => {
      const svg = d3.select("svg");
      const width = +svg.attr("width");
      const height = +svg.attr("height");

      const simulation = d3.forceSimulation(data.nodes)
        .force("link", d3.forceLink(data.links).id(d => d.id))
        .force("charge", d3.forceManyBody().strength(-300))
        .force("center", d3.forceCenter(width / 2, height / 2));

      const link = svg.append("g")
        .selectAll("line")
        .data(data.links)
        .enter().append("line")
        .attr("stroke-width", d => Math.sqrt(d.value));

      const node = svg.append("g")
        .selectAll("circle")
        .data(data.nodes)
        .enter().append("circle")
        .attr("r", 5)
        .call(d3.drag()
          .on("start", dragstarted)
          .on("drag", dragged)
          .on("end", dragended));

      node.append("title")
        .text(d => d.name);

      simulation.on("tick", () => {
        link
          .attr("x1", d => d.source.x)
          .attr("y1", d => d.source.y)
          .attr("x2", d => d.target.x)
          .attr("y2", d => d.target.y);

        node
          .attr("cx", d => d.x)
          .attr("cy", d => d.y);
      });

      function dragstarted(event) {
        if (!event.active) simulation.alphaTarget(0.3).restart();
        event.subject.fx = event.subject.x;
        event.subject.fy = event.subject.y;
      }

      function dragged(event) {
        event.subject.fx = event.x;
        event.subject.fy = event.y;
      }

      function dragended(event) {
        if (!event.active) simulation.alphaTarget(0);
        event.subject.fx = null;
        event.subject.fy = null;
      }
    });
  </script>
</body>
</html>
```

### 6. NetworkX Python Analysis

```typescript
import { exportToNetworkX } from 'ruvector-extensions';

const nxData = exportToNetworkX(graph);
await writeFile('graph.json', JSON.stringify(nxData, null, 2));
```

**Python Analysis:**

```python
import json
import networkx as nx
import matplotlib.pyplot as plt
import numpy as np

# Load graph
with open('graph.json', 'r') as f:
    data = json.load(f)

G = nx.node_link_graph(data)

print(f"Nodes: {G.number_of_nodes()}")
print(f"Edges: {G.number_of_edges()}")
print(f"Density: {nx.density(G):.4f}")

# Centrality analysis
degree_cent = nx.degree_centrality(G)
between_cent = nx.betweenness_centrality(G)
close_cent = nx.closeness_centrality(G)
eigen_cent = nx.eigenvector_centrality(G)

# Community detection
communities = nx.community.louvain_communities(G)
print(f"\nFound {len(communities)} communities")

# Visualize
plt.figure(figsize=(12, 8))
pos = nx.spring_layout(G, k=0.5, iterations=50)

# Color by community
color_map = []
for node in G:
    for i, comm in enumerate(communities):
        if node in comm:
            color_map.append(i)
            break

nx.draw(G, pos,
        node_color=color_map,
        node_size=[v * 1000 for v in degree_cent.values()],
        cmap=plt.cm.rainbow,
        with_labels=True,
        font_size=8,
        edge_color='gray',
        alpha=0.7)

plt.title('Network Graph with Communities')
plt.savefig('network.png', dpi=300, bbox_inches='tight')

# Export metrics
metrics = {
    'node': list(G.nodes()),
    'degree_centrality': [degree_cent[n] for n in G.nodes()],
    'betweenness_centrality': [between_cent[n] for n in G.nodes()],
    'closeness_centrality': [close_cent[n] for n in G.nodes()],
    'eigenvector_centrality': [eigen_cent[n] for n in G.nodes()]
}

import pandas as pd
df = pd.DataFrame(metrics)
df.to_csv('network_metrics.csv', index=False)
print("\nMetrics exported to network_metrics.csv")
```

## Streaming Exports for Large Graphs

When dealing with millions of nodes, use streaming exporters:

### GraphML Streaming

```typescript
import { GraphMLStreamExporter } from 'ruvector-extensions';
import { createWriteStream } from 'fs';

const stream = createWriteStream('large-graph.graphml');
const exporter = new GraphMLStreamExporter(stream, {
  graphName: 'Large Network'
});

await exporter.start();

// Add nodes in batches
for (const batch of nodeBatches) {
  for (const node of batch) {
    await exporter.addNode(node);
  }
  console.log(`Processed ${batch.length} nodes`);
}

// Add edges
for (const batch of edgeBatches) {
  for (const edge of batch) {
    await exporter.addEdge(edge);
  }
}

await exporter.end();
stream.close();
```

### D3.js Streaming

```typescript
import { D3StreamExporter } from 'ruvector-extensions';

const stream = createWriteStream('large-d3-graph.json');
const exporter = new D3StreamExporter(stream);

await exporter.start();

// Process in chunks
for await (const node of nodeIterator) {
  await exporter.addNode(node);
}

for await (const edge of edgeIterator) {
  await exporter.addEdge(edge);
}

await exporter.end();
```

## Configuration Options

### Export Options

```typescript
interface ExportOptions {
  includeVectors?: boolean;      // Include embeddings (default: false)
  includeMetadata?: boolean;     // Include node attributes (default: true)
  maxNeighbors?: number;         // Max edges per node (default: 10)
  threshold?: number;            // Min similarity (default: 0.0)
  graphName?: string;            // Graph title
  graphDescription?: string;     // Graph description
  streaming?: boolean;           // Enable streaming mode
  attributeMapping?: Record<string, string>; // Custom attribute names
}
```

### Graph Building Options

```typescript
const graph = buildGraphFromEntries(entries, {
  maxNeighbors: 5,        // Create at most 5 edges per node
  threshold: 0.7,         // Only connect if similarity > 0.7
  includeVectors: false,  // Don't export raw embeddings
  includeMetadata: true   // Export all metadata fields
});
```

## Performance Tips

1. **Threshold Selection**: Higher thresholds = fewer edges = smaller files
2. **maxNeighbors**: Limit connections per node for cleaner graphs
3. **Streaming**: Use for graphs > 100K nodes
4. **Compression**: Compress output files (gzip recommended)
5. **Batch Processing**: Process nodes/edges in batches

## Use Cases

### 1. Document Similarity Network

```typescript
const docs = await embedDocuments(documents);
const graph = buildGraphFromEntries(docs, {
  threshold: 0.8,
  maxNeighbors: 5
});

const gexf = exportToGEXF(graph);
// Visualize in Gephi to find document clusters
```

### 2. Knowledge Graph

```typescript
const concepts = await embedConcepts(knowledgeBase);
const graph = buildGraphFromEntries(concepts, {
  threshold: 0.6,
  includeMetadata: true
});

const cypher = exportToNeo4j(graph);
// Import into Neo4j for graph queries
```

### 3. Semantic Search Visualization

```typescript
const results = db.search({ vector: queryVector, k: 50 });
const graph = buildGraphFromEntries(results, {
  maxNeighbors: 3,
  threshold: 0.5
});

const d3Data = exportToD3(graph);
// Show interactive graph in web app
```

### 4. Research Network Analysis

```typescript
const papers = await embedPapers(corpus);
const graph = buildGraphFromEntries(papers, {
  threshold: 0.75,
  includeMetadata: true
});

const nxData = exportToNetworkX(graph);
// Analyze citation patterns, communities, and influence in Python
```

## Troubleshooting

### Large Graphs Won't Export

**Problem**: Out of memory errors with large graphs.

**Solution**: Use streaming exporters:

```typescript
const exporter = new GraphMLStreamExporter(stream);
await exporter.start();
// Process in batches
await exporter.end();
```

### Neo4j Import Fails

**Problem**: Cypher queries fail or timeout.

**Solution**: Break into batches:

```typescript
// Export in batches of 1000 nodes
const batches = chunkArray(graph.nodes, 1000);
for (const batch of batches) {
  const batchGraph = { nodes: batch, edges: filterEdges(batch) };
  const cypher = exportToNeo4j(batchGraph);
  await neo4jSession.run(cypher);
}
```

### Gephi Import Issues

**Problem**: Attributes not showing correctly.

**Solution**: Ensure metadata is included:

```typescript
const gexf = exportToGEXF(graph, {
  includeMetadata: true,  // ✓ Include all attributes
  graphName: 'My Network'
});
```

### D3.js Performance

**Problem**: Web visualization lags with many nodes.

**Solution**: Limit nodes or use clustering:

```typescript
// Filter to top nodes only
const topNodes = graph.nodes.slice(0, 100);
const filteredGraph = {
  nodes: topNodes,
  edges: graph.edges.filter(e =>
    topNodes.some(n => n.id === e.source || n.id === e.target)
  )
};

const d3Data = exportToD3(filteredGraph);
```

## Best Practices

1. **Choose the Right Format**:
   - GraphML: General purpose, wide tool support
   - GEXF: Best for Gephi visualization
   - Neo4j: For graph database queries
   - D3.js: Interactive web visualization
   - NetworkX: Python analysis

2. **Optimize Graph Size**:
   - Use threshold to reduce edges
   - Limit maxNeighbors
   - Filter out low-quality connections

3. **Preserve Metadata**:
   - Always include relevant metadata
   - Use descriptive labels
   - Add timestamps for temporal analysis

4. **Test with Small Samples**:
   - Export a subset first
   - Verify format compatibility
   - Check visualization quality

5. **Document Your Process**:
   - Record threshold and parameters
   - Save graph statistics
   - Version your exports

## Additional Resources

- [GraphML Specification](http://graphml.graphdrawing.org/)
- [GEXF Format Documentation](https://gephi.org/gexf/format/)
- [Neo4j Cypher Manual](https://neo4j.com/docs/cypher-manual/)
- [D3.js Force Layout](https://d3js.org/d3-force)
- [NetworkX Documentation](https://networkx.org/documentation/)

## Support

For issues and questions:
- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://github.com/ruvnet/ruvector
- Examples: See `examples/graph-export-examples.ts`
