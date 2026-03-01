# Graph Exporters API Reference

Complete API documentation for the ruvector-extensions graph export module.

## Table of Contents

- [Graph Building](#graph-building)
- [Export Functions](#export-functions)
- [Streaming Exporters](#streaming-exporters)
- [Types and Interfaces](#types-and-interfaces)
- [Utilities](#utilities)

## Graph Building

### buildGraphFromEntries()

Build a graph from an array of vector entries by computing similarity.

```typescript
function buildGraphFromEntries(
  entries: VectorEntry[],
  options?: ExportOptions
): Graph
```

**Parameters:**
- `entries: VectorEntry[]` - Array of vector entries with id, vector, and optional metadata
- `options?: ExportOptions` - Configuration options

**Returns:** `Graph` - Graph structure with nodes and edges

**Example:**

```typescript
const entries = [
  { id: 'doc1', vector: [0.1, 0.2, 0.3], metadata: { title: 'AI' } },
  { id: 'doc2', vector: [0.15, 0.25, 0.35], metadata: { title: 'ML' } }
];

const graph = buildGraphFromEntries(entries, {
  maxNeighbors: 5,
  threshold: 0.7,
  includeMetadata: true
});
```

### buildGraphFromVectorDB()

Build a graph directly from a VectorDB instance.

```typescript
function buildGraphFromVectorDB(
  db: VectorDB,
  options?: ExportOptions
): Graph
```

**Note:** Currently throws an error as VectorDB doesn't expose a list() method. Use `buildGraphFromEntries()` instead with pre-fetched entries.

## Export Functions

### exportGraph()

Universal export function that routes to the appropriate format exporter.

```typescript
function exportGraph(
  graph: Graph,
  format: ExportFormat,
  options?: ExportOptions
): ExportResult
```

**Parameters:**
- `graph: Graph` - Graph to export
- `format: ExportFormat` - Target format ('graphml' | 'gexf' | 'neo4j' | 'd3' | 'networkx')
- `options?: ExportOptions` - Export configuration

**Returns:** `ExportResult` - Export result with data and metadata

**Example:**

```typescript
const result = exportGraph(graph, 'graphml', {
  graphName: 'My Network',
  includeMetadata: true
});

console.log(result.data); // GraphML XML string
console.log(result.nodeCount); // Number of nodes
console.log(result.edgeCount); // Number of edges
```

### exportToGraphML()

Export graph to GraphML XML format.

```typescript
function exportToGraphML(
  graph: Graph,
  options?: ExportOptions
): string
```

**Returns:** GraphML XML string

**Features:**
- XML-based format
- Supported by Gephi, yEd, NetworkX, igraph, Cytoscape
- Includes node and edge attributes
- Proper XML escaping

**Example:**

```typescript
const graphml = exportToGraphML(graph, {
  graphName: 'Document Network',
  includeVectors: false,
  includeMetadata: true
});

await writeFile('network.graphml', graphml);
```

### exportToGEXF()

Export graph to GEXF XML format (optimized for Gephi).

```typescript
function exportToGEXF(
  graph: Graph,
  options?: ExportOptions
): string
```

**Returns:** GEXF XML string

**Features:**
- Designed for Gephi
- Rich metadata support
- Includes graph description and creator info
- Timestamp-based versioning

**Example:**

```typescript
const gexf = exportToGEXF(graph, {
  graphName: 'Knowledge Graph',
  graphDescription: 'Vector similarity network',
  includeMetadata: true
});

await writeFile('network.gexf', gexf);
```

### exportToNeo4j()

Export graph to Neo4j Cypher queries.

```typescript
function exportToNeo4j(
  graph: Graph,
  options?: ExportOptions
): string
```

**Returns:** Cypher query string

**Features:**
- CREATE statements for nodes
- MATCH/CREATE for relationships
- Constraints and indexes
- Verification queries
- Proper Cypher escaping

**Example:**

```typescript
const cypher = exportToNeo4j(graph, {
  includeVectors: true,
  includeMetadata: true
});

// Execute in Neo4j
await neo4jSession.run(cypher);
```

### exportToNeo4jJSON()

Export graph to Neo4j JSON import format.

```typescript
function exportToNeo4jJSON(
  graph: Graph,
  options?: ExportOptions
): { nodes: any[]; relationships: any[] }
```

**Returns:** Object with nodes and relationships arrays

**Example:**

```typescript
const neoData = exportToNeo4jJSON(graph);
await writeFile('neo4j-import.json', JSON.stringify(neoData));
```

### exportToD3()

Export graph to D3.js JSON format.

```typescript
function exportToD3(
  graph: Graph,
  options?: ExportOptions
): { nodes: any[]; links: any[] }
```

**Returns:** Object with nodes and links arrays

**Features:**
- Compatible with D3.js force simulation
- Node attributes preserved
- Link weights as values
- Ready for web visualization

**Example:**

```typescript
const d3Data = exportToD3(graph, {
  includeMetadata: true
});

// Use in D3.js
const simulation = d3.forceSimulation(d3Data.nodes)
  .force("link", d3.forceLink(d3Data.links).id(d => d.id));
```

### exportToD3Hierarchy()

Export graph to D3.js hierarchy format for tree layouts.

```typescript
function exportToD3Hierarchy(
  graph: Graph,
  rootId: string,
  options?: ExportOptions
): any
```

**Parameters:**
- `rootId: string` - ID of the root node

**Returns:** Hierarchical JSON object

**Example:**

```typescript
const hierarchy = exportToD3Hierarchy(graph, 'root-node', {
  includeMetadata: true
});

// Use with D3 tree layout
const root = d3.hierarchy(hierarchy);
const treeLayout = d3.tree()(root);
```

### exportToNetworkX()

Export graph to NetworkX node-link JSON format.

```typescript
function exportToNetworkX(
  graph: Graph,
  options?: ExportOptions
): any
```

**Returns:** NetworkX-compatible JSON object

**Features:**
- Node-link format
- Directed graph support
- Full metadata preservation
- Compatible with nx.node_link_graph()

**Example:**

```typescript
const nxData = exportToNetworkX(graph);
await writeFile('graph.json', JSON.stringify(nxData));
```

Python usage:

```python
import networkx as nx
import json

with open('graph.json') as f:
    data = json.load(f)

G = nx.node_link_graph(data)
```

### exportToNetworkXEdgeList()

Export graph to NetworkX edge list format.

```typescript
function exportToNetworkXEdgeList(graph: Graph): string
```

**Returns:** Edge list string (one edge per line)

**Format:** `source target weight`

**Example:**

```typescript
const edgeList = exportToNetworkXEdgeList(graph);
await writeFile('edges.txt', edgeList);
```

### exportToNetworkXAdjacencyList()

Export graph to NetworkX adjacency list format.

```typescript
function exportToNetworkXAdjacencyList(graph: Graph): string
```

**Returns:** Adjacency list string

**Format:** `source target1:weight1 target2:weight2 ...`

**Example:**

```typescript
const adjList = exportToNetworkXAdjacencyList(graph);
await writeFile('adjacency.txt', adjList);
```

## Streaming Exporters

For large graphs that don't fit in memory, use streaming exporters.

### GraphMLStreamExporter

Stream large graphs to GraphML format.

```typescript
class GraphMLStreamExporter extends StreamingExporter {
  constructor(stream: Writable, options?: ExportOptions)

  async start(): Promise<void>
  async addNode(node: GraphNode): Promise<void>
  async addEdge(edge: GraphEdge): Promise<void>
  async end(): Promise<void>
}
```

**Example:**

```typescript
import { createWriteStream } from 'fs';

const stream = createWriteStream('large-graph.graphml');
const exporter = new GraphMLStreamExporter(stream, {
  graphName: 'Large Network'
});

await exporter.start();

// Add nodes
for (const node of nodes) {
  await exporter.addNode(node);
}

// Add edges
for (const edge of edges) {
  await exporter.addEdge(edge);
}

await exporter.end();
stream.close();
```

### D3StreamExporter

Stream large graphs to D3.js JSON format.

```typescript
class D3StreamExporter extends StreamingExporter {
  constructor(stream: Writable, options?: ExportOptions)

  async start(): Promise<void>
  async addNode(node: GraphNode): Promise<void>
  async addEdge(edge: GraphEdge): Promise<void>
  async end(): Promise<void>
}
```

**Example:**

```typescript
const stream = createWriteStream('large-d3-graph.json');
const exporter = new D3StreamExporter(stream);

await exporter.start();

for (const node of nodeGenerator()) {
  await exporter.addNode(node);
}

for (const edge of edgeGenerator()) {
  await exporter.addEdge(edge);
}

await exporter.end();
```

### streamToGraphML()

Helper function for streaming GraphML export.

```typescript
async function streamToGraphML(
  graph: Graph,
  stream: Writable,
  options?: ExportOptions
): Promise<void>
```

## Types and Interfaces

### Graph

Complete graph structure.

```typescript
interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  metadata?: Record<string, any>;
}
```

### GraphNode

Graph node representing a vector entry.

```typescript
interface GraphNode {
  id: string;
  label?: string;
  vector?: number[];
  attributes?: Record<string, any>;
}
```

### GraphEdge

Graph edge representing similarity between nodes.

```typescript
interface GraphEdge {
  source: string;
  target: string;
  weight: number;
  type?: string;
  attributes?: Record<string, any>;
}
```

### ExportOptions

Configuration options for exports.

```typescript
interface ExportOptions {
  includeVectors?: boolean;        // Include embeddings (default: false)
  includeMetadata?: boolean;       // Include attributes (default: true)
  maxNeighbors?: number;           // Max edges per node (default: 10)
  threshold?: number;              // Min similarity (default: 0.0)
  graphName?: string;              // Graph title
  graphDescription?: string;       // Graph description
  streaming?: boolean;             // Enable streaming
  attributeMapping?: Record<string, string>; // Custom mappings
}
```

### ExportFormat

Supported export format types.

```typescript
type ExportFormat = 'graphml' | 'gexf' | 'neo4j' | 'd3' | 'networkx';
```

### ExportResult

Export result containing output and metadata.

```typescript
interface ExportResult {
  format: ExportFormat;
  data: string | object;
  nodeCount: number;
  edgeCount: number;
  metadata?: Record<string, any>;
}
```

## Utilities

### validateGraph()

Validate graph structure and throw errors if invalid.

```typescript
function validateGraph(graph: Graph): void
```

**Checks:**
- Nodes array exists
- Edges array exists
- All nodes have IDs
- All edges reference existing nodes
- All edges have numeric weights

**Example:**

```typescript
try {
  validateGraph(graph);
  console.log('Graph is valid');
} catch (error) {
  console.error('Invalid graph:', error.message);
}
```

### cosineSimilarity()

Compute cosine similarity between two vectors.

```typescript
function cosineSimilarity(a: number[], b: number[]): number
```

**Returns:** Similarity score (0-1, higher is better)

**Example:**

```typescript
const sim = cosineSimilarity([1, 0, 0], [0.9, 0.1, 0]);
console.log(sim); // ~0.995
```

## Error Handling

All functions may throw errors:

```typescript
try {
  const graph = buildGraphFromEntries(entries);
  const result = exportGraph(graph, 'graphml');
} catch (error) {
  if (error.message.includes('dimension')) {
    console.error('Vector dimension mismatch');
  } else if (error.message.includes('format')) {
    console.error('Unsupported export format');
  } else {
    console.error('Export failed:', error);
  }
}
```

## Performance Notes

- **Memory**: Streaming exporters use constant memory
- **Speed**: Binary formats faster than XML
- **Threshold**: Higher thresholds = fewer edges = faster exports
- **maxNeighbors**: Limiting neighbors reduces graph size
- **Batch Processing**: Process large datasets in chunks

## Browser Support

The module is designed for Node.js. For browser use:

1. Use bundlers (webpack, Rollup)
2. Polyfill Node.js streams
3. Use web-friendly formats (D3.js JSON)

## Version Compatibility

- Node.js ≥ 18.0.0
- TypeScript ≥ 5.0
- ruvector ≥ 0.1.0

## License

MIT - See LICENSE file for details
