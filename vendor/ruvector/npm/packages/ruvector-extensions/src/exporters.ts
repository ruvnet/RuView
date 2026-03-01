/**
 * Graph Export Module for ruvector-extensions
 *
 * Provides export functionality to multiple graph formats:
 * - GraphML (XML-based graph format)
 * - GEXF (Graph Exchange XML Format for Gephi)
 * - Neo4j (Cypher queries)
 * - D3.js JSON (web visualization)
 * - NetworkX (Python graph library)
 *
 * Features:
 * - Full TypeScript types and interfaces
 * - Streaming exports for large graphs
 * - Configurable export options
 * - Support for node attributes and edge weights
 * - Error handling and validation
 *
 * @module exporters
 */

import { Writable } from 'stream';
import type { VectorEntry, SearchResult } from 'ruvector';

// VectorDB is a class, not a type
type VectorDBInstance = any;

// ============================================================================
// Type Definitions
// ============================================================================

/**
 * Graph node representing a vector entry
 */
export interface GraphNode {
  /** Unique node identifier */
  id: string;
  /** Node label/name */
  label?: string;
  /** Vector embedding */
  vector?: number[];
  /** Node attributes/metadata */
  attributes?: Record<string, any>;
}

/**
 * Graph edge representing similarity between nodes
 */
export interface GraphEdge {
  /** Source node ID */
  source: string;
  /** Target node ID */
  target: string;
  /** Edge weight (similarity score) */
  weight: number;
  /** Edge type/label */
  type?: string;
  /** Edge attributes */
  attributes?: Record<string, any>;
}

/**
 * Complete graph structure
 */
export interface Graph {
  /** Graph nodes */
  nodes: GraphNode[];
  /** Graph edges */
  edges: GraphEdge[];
  /** Graph-level metadata */
  metadata?: Record<string, any>;
}

/**
 * Export configuration options
 */
export interface ExportOptions {
  /** Include vector embeddings in export */
  includeVectors?: boolean;
  /** Include metadata/attributes */
  includeMetadata?: boolean;
  /** Maximum number of neighbors per node */
  maxNeighbors?: number;
  /** Minimum similarity threshold for edges */
  threshold?: number;
  /** Graph title/name */
  graphName?: string;
  /** Graph description */
  graphDescription?: string;
  /** Enable streaming mode for large graphs */
  streaming?: boolean;
  /** Custom attribute mappings */
  attributeMapping?: Record<string, string>;
}

/**
 * Export format types
 */
export type ExportFormat = 'graphml' | 'gexf' | 'neo4j' | 'd3' | 'networkx';

/**
 * Export result containing output and metadata
 */
export interface ExportResult {
  /** Export format used */
  format: ExportFormat;
  /** Exported data (string or object depending on format) */
  data: string | object;
  /** Number of nodes exported */
  nodeCount: number;
  /** Number of edges exported */
  edgeCount: number;
  /** Export metadata */
  metadata?: Record<string, any>;
}

// ============================================================================
// Graph Builder
// ============================================================================

/**
 * Build a graph from VectorDB by computing similarity between vectors
 *
 * @param db - VectorDB instance
 * @param options - Export options
 * @returns Graph structure
 *
 * @example
 * ```typescript
 * const graph = buildGraphFromVectorDB(db, {
 *   maxNeighbors: 5,
 *   threshold: 0.7,
 *   includeVectors: false
 * });
 * ```
 */
export function buildGraphFromVectorDB(
  db: VectorDBInstance,
  options: ExportOptions = {}
): Graph {
  const {
    maxNeighbors = 10,
    threshold = 0.0,
    includeVectors = false,
    includeMetadata = true
  } = options;

  const stats = db.stats();
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];
  const processedIds = new Set<string>();

  // Get all vectors by searching with a dummy query
  // Since we don't have a list() method, we'll need to build the graph incrementally
  // This is a limitation - in practice, you'd want to add a list() or getAllIds() method to VectorDB

  // For now, we'll create a helper function that needs to be called with pre-fetched entries
  throw new Error(
    'buildGraphFromVectorDB requires VectorDB to have a list() or getAllIds() method. ' +
    'Please use buildGraphFromEntries() instead with pre-fetched vector entries.'
  );
}

/**
 * Build a graph from a list of vector entries
 *
 * @param entries - Array of vector entries
 * @param options - Export options
 * @returns Graph structure
 *
 * @example
 * ```typescript
 * const entries = [...]; // Your vector entries
 * const graph = buildGraphFromEntries(entries, {
 *   maxNeighbors: 5,
 *   threshold: 0.7
 * });
 * ```
 */
export function buildGraphFromEntries(
  entries: VectorEntry[],
  options: ExportOptions = {}
): Graph {
  const {
    maxNeighbors = 10,
    threshold = 0.0,
    includeVectors = false,
    includeMetadata = true
  } = options;

  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  // Create nodes
  for (const entry of entries) {
    const node: GraphNode = {
      id: entry.id,
      label: entry.metadata?.name || entry.metadata?.label || entry.id
    };

    if (includeVectors) {
      node.vector = entry.vector;
    }

    if (includeMetadata && entry.metadata) {
      node.attributes = { ...entry.metadata };
    }

    nodes.push(node);
  }

  // Create edges by computing similarity
  for (let i = 0; i < entries.length; i++) {
    const neighbors: Array<{ index: number; similarity: number }> = [];

    for (let j = 0; j < entries.length; j++) {
      if (i === j) continue;

      const similarity = cosineSimilarity(entries[i].vector, entries[j].vector);

      if (similarity >= threshold) {
        neighbors.push({ index: j, similarity });
      }
    }

    // Sort by similarity and take top k
    neighbors.sort((a, b) => b.similarity - a.similarity);
    const topNeighbors = neighbors.slice(0, maxNeighbors);

    // Create edges
    for (const neighbor of topNeighbors) {
      edges.push({
        source: entries[i].id,
        target: entries[neighbor.index].id,
        weight: neighbor.similarity,
        type: 'similarity'
      });
    }
  }

  return {
    nodes,
    edges,
    metadata: {
      nodeCount: nodes.length,
      edgeCount: edges.length,
      threshold,
      maxNeighbors
    }
  };
}

/**
 * Compute cosine similarity between two vectors
 */
function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length) {
    throw new Error('Vectors must have the same dimension');
  }

  let dotProduct = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  normA = Math.sqrt(normA);
  normB = Math.sqrt(normB);

  if (normA === 0 || normB === 0) {
    return 0;
  }

  return dotProduct / (normA * normB);
}

// ============================================================================
// GraphML Exporter
// ============================================================================

/**
 * Export graph to GraphML format (XML-based)
 *
 * GraphML is a comprehensive and easy-to-use file format for graphs.
 * It's supported by many graph analysis tools including Gephi, NetworkX, and igraph.
 *
 * @param graph - Graph to export
 * @param options - Export options
 * @returns GraphML XML string
 *
 * @example
 * ```typescript
 * const graphml = exportToGraphML(graph, {
 *   graphName: 'Vector Similarity Graph',
 *   includeVectors: false
 * });
 * console.log(graphml);
 * ```
 */
export function exportToGraphML(
  graph: Graph,
  options: ExportOptions = {}
): string {
  const { graphName = 'VectorGraph', includeVectors = false } = options;

  let xml = '<?xml version="1.0" encoding="UTF-8"?>\n';
  xml += '<graphml xmlns="http://graphml.graphdrawing.org/xmlns"\n';
  xml += '         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"\n';
  xml += '         xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns\n';
  xml += '         http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">\n\n';

  // Define node attributes
  xml += '  <!-- Node attributes -->\n';
  xml += '  <key id="label" for="node" attr.name="label" attr.type="string"/>\n';

  if (includeVectors) {
    xml += '  <key id="vector" for="node" attr.name="vector" attr.type="string"/>\n';
  }

  // Collect all unique node attributes
  const nodeAttrs = new Set<string>();
  for (const node of graph.nodes) {
    if (node.attributes) {
      Object.keys(node.attributes).forEach(key => nodeAttrs.add(key));
    }
  }

  Array.from(nodeAttrs).forEach(attr => {
    xml += `  <key id="node_${escapeXML(attr)}" for="node" attr.name="${escapeXML(attr)}" attr.type="string"/>\n`;
  });

  // Define edge attributes
  xml += '\n  <!-- Edge attributes -->\n';
  xml += '  <key id="weight" for="edge" attr.name="weight" attr.type="double"/>\n';
  xml += '  <key id="type" for="edge" attr.name="type" attr.type="string"/>\n';

  // Start graph
  xml += `\n  <graph id="${escapeXML(graphName)}" edgedefault="directed">\n\n`;

  // Add nodes
  xml += '    <!-- Nodes -->\n';
  for (const node of graph.nodes) {
    xml += `    <node id="${escapeXML(node.id)}">\n`;

    if (node.label) {
      xml += `      <data key="label">${escapeXML(node.label)}</data>\n`;
    }

    if (includeVectors && node.vector) {
      xml += `      <data key="vector">${escapeXML(JSON.stringify(node.vector))}</data>\n`;
    }

    if (node.attributes) {
      for (const [key, value] of Object.entries(node.attributes)) {
        xml += `      <data key="node_${escapeXML(key)}">${escapeXML(String(value))}</data>\n`;
      }
    }

    xml += '    </node>\n';
  }

  // Add edges
  xml += '\n    <!-- Edges -->\n';
  for (let i = 0; i < graph.edges.length; i++) {
    const edge = graph.edges[i];
    xml += `    <edge id="e${i}" source="${escapeXML(edge.source)}" target="${escapeXML(edge.target)}">\n`;
    xml += `      <data key="weight">${edge.weight}</data>\n`;

    if (edge.type) {
      xml += `      <data key="type">${escapeXML(edge.type)}</data>\n`;
    }

    xml += '    </edge>\n';
  }

  xml += '  </graph>\n';
  xml += '</graphml>\n';

  return xml;
}

/**
 * Stream graph to GraphML format
 *
 * @param graph - Graph to export
 * @param stream - Writable stream
 * @param options - Export options
 *
 * @example
 * ```typescript
 * import { createWriteStream } from 'fs';
 * const stream = createWriteStream('graph.graphml');
 * await streamToGraphML(graph, stream);
 * ```
 */
export async function streamToGraphML(
  graph: Graph,
  stream: Writable,
  options: ExportOptions = {}
): Promise<void> {
  const graphml = exportToGraphML(graph, options);

  return new Promise((resolve, reject) => {
    stream.write(graphml, (err) => {
      if (err) reject(err);
      else resolve();
    });
  });
}

// ============================================================================
// GEXF Exporter
// ============================================================================

/**
 * Export graph to GEXF format (Gephi)
 *
 * GEXF (Graph Exchange XML Format) is designed for Gephi, a popular
 * graph visualization tool. It supports rich graph attributes and dynamics.
 *
 * @param graph - Graph to export
 * @param options - Export options
 * @returns GEXF XML string
 *
 * @example
 * ```typescript
 * const gexf = exportToGEXF(graph, {
 *   graphName: 'Vector Network',
 *   graphDescription: 'Similarity network of embeddings'
 * });
 * ```
 */
export function exportToGEXF(
  graph: Graph,
  options: ExportOptions = {}
): string {
  const {
    graphName = 'VectorGraph',
    graphDescription = 'Vector similarity graph',
    includeVectors = false
  } = options;

  const timestamp = new Date().toISOString();

  let xml = '<?xml version="1.0" encoding="UTF-8"?>\n';
  xml += '<gexf xmlns="http://www.gexf.net/1.2draft"\n';
  xml += '      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"\n';
  xml += '      xsi:schemaLocation="http://www.gexf.net/1.2draft\n';
  xml += '      http://www.gexf.net/1.2draft/gexf.xsd"\n';
  xml += '      version="1.2">\n\n';

  xml += `  <meta lastmodifieddate="${timestamp}">\n`;
  xml += `    <creator>ruvector-extensions</creator>\n`;
  xml += `    <description>${escapeXML(graphDescription)}</description>\n`;
  xml += '  </meta>\n\n';

  xml += '  <graph mode="static" defaultedgetype="directed">\n\n';

  // Node attributes
  xml += '    <attributes class="node">\n';
  xml += '      <attribute id="0" title="label" type="string"/>\n';

  let attrId = 1;
  const nodeAttrMap = new Map<string, number>();

  if (includeVectors) {
    xml += `      <attribute id="${attrId}" title="vector" type="string"/>\n`;
    nodeAttrMap.set('vector', attrId++);
  }

  // Collect unique node attributes
  const nodeAttrs = new Set<string>();
  for (const node of graph.nodes) {
    if (node.attributes) {
      Object.keys(node.attributes).forEach(key => nodeAttrs.add(key));
    }
  }

  Array.from(nodeAttrs).forEach(attr => {
    xml += `      <attribute id="${attrId}" title="${escapeXML(attr)}" type="string"/>\n`;
    nodeAttrMap.set(attr, attrId++);
  });

  xml += '    </attributes>\n\n';

  // Edge attributes
  xml += '    <attributes class="edge">\n';
  xml += '      <attribute id="0" title="weight" type="double"/>\n';
  xml += '      <attribute id="1" title="type" type="string"/>\n';
  xml += '    </attributes>\n\n';

  // Nodes
  xml += '    <nodes>\n';
  for (const node of graph.nodes) {
    xml += `      <node id="${escapeXML(node.id)}" label="${escapeXML(node.label || node.id)}">\n`;
    xml += '        <attvalues>\n';

    if (includeVectors && node.vector) {
      const vectorId = nodeAttrMap.get('vector');
      xml += `          <attvalue for="${vectorId}" value="${escapeXML(JSON.stringify(node.vector))}"/>\n`;
    }

    if (node.attributes) {
      for (const [key, value] of Object.entries(node.attributes)) {
        const attrIdForKey = nodeAttrMap.get(key);
        if (attrIdForKey !== undefined) {
          xml += `          <attvalue for="${attrIdForKey}" value="${escapeXML(String(value))}"/>\n`;
        }
      }
    }

    xml += '        </attvalues>\n';
    xml += '      </node>\n';
  }
  xml += '    </nodes>\n\n';

  // Edges
  xml += '    <edges>\n';
  for (let i = 0; i < graph.edges.length; i++) {
    const edge = graph.edges[i];
    xml += `      <edge id="${i}" source="${escapeXML(edge.source)}" target="${escapeXML(edge.target)}" weight="${edge.weight}">\n`;
    xml += '        <attvalues>\n';
    xml += `          <attvalue for="0" value="${edge.weight}"/>\n`;

    if (edge.type) {
      xml += `          <attvalue for="1" value="${escapeXML(edge.type)}"/>\n`;
    }

    xml += '        </attvalues>\n';
    xml += '      </edge>\n';
  }
  xml += '    </edges>\n\n';

  xml += '  </graph>\n';
  xml += '</gexf>\n';

  return xml;
}

// ============================================================================
// Neo4j Exporter
// ============================================================================

/**
 * Export graph to Neo4j Cypher queries
 *
 * Generates Cypher CREATE statements that can be executed in Neo4j
 * to import the graph structure.
 *
 * @param graph - Graph to export
 * @param options - Export options
 * @returns Cypher query string
 *
 * @example
 * ```typescript
 * const cypher = exportToNeo4j(graph, {
 *   includeVectors: true,
 *   includeMetadata: true
 * });
 * // Execute in Neo4j shell or driver
 * ```
 */
export function exportToNeo4j(
  graph: Graph,
  options: ExportOptions = {}
): string {
  const { includeVectors = false, includeMetadata = true } = options;

  let cypher = '// Neo4j Cypher Import Script\n';
  cypher += '// Generated by ruvector-extensions\n\n';
  cypher += '// Clear existing data (optional - uncomment if needed)\n';
  cypher += '// MATCH (n) DETACH DELETE n;\n\n';

  // Create constraint for unique IDs
  cypher += '// Create constraint for unique node IDs\n';
  cypher += 'CREATE CONSTRAINT vector_id IF NOT EXISTS FOR (v:Vector) REQUIRE v.id IS UNIQUE;\n\n';

  // Create nodes
  cypher += '// Create nodes\n';
  for (const node of graph.nodes) {
    const props: string[] = [`id: "${escapeCypher(node.id)}"`];

    if (node.label) {
      props.push(`label: "${escapeCypher(node.label)}"`);
    }

    if (includeVectors && node.vector) {
      props.push(`vector: [${node.vector.join(', ')}]`);
    }

    if (includeMetadata && node.attributes) {
      for (const [key, value] of Object.entries(node.attributes)) {
        const cypherValue = typeof value === 'string'
          ? `"${escapeCypher(value)}"`
          : JSON.stringify(value);
        props.push(`${escapeCypher(key)}: ${cypherValue}`);
      }
    }

    cypher += `CREATE (:Vector {${props.join(', ')}});\n`;
  }

  cypher += '\n// Create relationships\n';

  // Create edges
  for (const edge of graph.edges) {
    const relType = edge.type ? escapeCypher(edge.type.toUpperCase()) : 'SIMILAR_TO';
    cypher += `MATCH (a:Vector {id: "${escapeCypher(edge.source)}"}), `;
    cypher += `(b:Vector {id: "${escapeCypher(edge.target)}"})\n`;
    cypher += `CREATE (a)-[:${relType} {weight: ${edge.weight}}]->(b);\n`;
  }

  cypher += '\n// Create indexes for performance\n';
  cypher += 'CREATE INDEX vector_label IF NOT EXISTS FOR (v:Vector) ON (v.label);\n\n';

  cypher += '// Verify import\n';
  cypher += 'MATCH (n:Vector) RETURN count(n) as nodeCount;\n';
  cypher += 'MATCH ()-[r]->() RETURN count(r) as edgeCount;\n';

  return cypher;
}

/**
 * Export graph to Neo4j JSON format (for neo4j-admin import)
 *
 * @param graph - Graph to export
 * @param options - Export options
 * @returns Neo4j JSON import format
 */
export function exportToNeo4jJSON(
  graph: Graph,
  options: ExportOptions = {}
): { nodes: any[]; relationships: any[] } {
  const { includeVectors = false, includeMetadata = true } = options;

  const nodes = graph.nodes.map(node => {
    const props: any = { id: node.id };

    if (node.label) props.label = node.label;
    if (includeVectors && node.vector) props.vector = node.vector;
    if (includeMetadata && node.attributes) Object.assign(props, node.attributes);

    return {
      type: 'node',
      id: node.id,
      labels: ['Vector'],
      properties: props
    };
  });

  const relationships = graph.edges.map((edge, i) => ({
    type: 'relationship',
    id: `e${i}`,
    label: edge.type || 'SIMILAR_TO',
    start: {
      id: edge.source,
      labels: ['Vector']
    },
    end: {
      id: edge.target,
      labels: ['Vector']
    },
    properties: {
      weight: edge.weight,
      ...(edge.attributes || {})
    }
  }));

  return { nodes, relationships };
}

// ============================================================================
// D3.js Exporter
// ============================================================================

/**
 * Export graph to D3.js JSON format
 *
 * Creates a JSON structure suitable for D3.js force-directed graphs
 * and other D3 visualizations.
 *
 * @param graph - Graph to export
 * @param options - Export options
 * @returns D3.js compatible JSON object
 *
 * @example
 * ```typescript
 * const d3Graph = exportToD3(graph);
 * // Use in D3.js force simulation
 * const simulation = d3.forceSimulation(d3Graph.nodes)
 *   .force("link", d3.forceLink(d3Graph.links));
 * ```
 */
export function exportToD3(
  graph: Graph,
  options: ExportOptions = {}
): { nodes: any[]; links: any[] } {
  const { includeVectors = false, includeMetadata = true } = options;

  const nodes = graph.nodes.map(node => {
    const d3Node: any = {
      id: node.id,
      name: node.label || node.id
    };

    if (includeVectors && node.vector) {
      d3Node.vector = node.vector;
    }

    if (includeMetadata && node.attributes) {
      Object.assign(d3Node, node.attributes);
    }

    return d3Node;
  });

  const links = graph.edges.map(edge => ({
    source: edge.source,
    target: edge.target,
    value: edge.weight,
    type: edge.type || 'similarity',
    ...(edge.attributes || {})
  }));

  return { nodes, links };
}

/**
 * Export graph to D3.js hierarchy format
 *
 * Creates a hierarchical JSON structure for D3.js tree layouts.
 * Requires a root node to be specified.
 *
 * @param graph - Graph to export
 * @param rootId - ID of the root node
 * @param options - Export options
 * @returns D3.js hierarchy object
 */
export function exportToD3Hierarchy(
  graph: Graph,
  rootId: string,
  options: ExportOptions = {}
): any {
  const { includeMetadata = true } = options;

  // Build adjacency map
  const adjacency = new Map<string, Set<string>>();
  for (const edge of graph.edges) {
    if (!adjacency.has(edge.source)) {
      adjacency.set(edge.source, new Set());
    }
    adjacency.get(edge.source)!.add(edge.target);
  }

  // Find node by ID
  const nodeMap = new Map(graph.nodes.map(n => [n.id, n]));
  const visited = new Set<string>();

  function buildHierarchy(nodeId: string): any {
    if (visited.has(nodeId)) return null;
    visited.add(nodeId);

    const node = nodeMap.get(nodeId);
    if (!node) return null;

    const hierarchyNode: any = {
      name: node.label || node.id,
      id: node.id
    };

    if (includeMetadata && node.attributes) {
      Object.assign(hierarchyNode, node.attributes);
    }

    const children = adjacency.get(nodeId);
    if (children && children.size > 0) {
      hierarchyNode.children = Array.from(children)
        .map(childId => buildHierarchy(childId))
        .filter(child => child !== null);
    }

    return hierarchyNode;
  }

  return buildHierarchy(rootId);
}

// ============================================================================
// NetworkX Exporter
// ============================================================================

/**
 * Export graph to NetworkX JSON format
 *
 * Creates node-link JSON format compatible with NetworkX's
 * node_link_graph() function.
 *
 * @param graph - Graph to export
 * @param options - Export options
 * @returns NetworkX JSON object
 *
 * @example
 * ```typescript
 * const nxGraph = exportToNetworkX(graph);
 * // In Python:
 * // import json
 * // import networkx as nx
 * // with open('graph.json') as f:
 * //     G = nx.node_link_graph(json.load(f))
 * ```
 */
export function exportToNetworkX(
  graph: Graph,
  options: ExportOptions = {}
): any {
  const { includeVectors = false, includeMetadata = true } = options;

  const nodes = graph.nodes.map(node => {
    const nxNode: any = { id: node.id };

    if (node.label) nxNode.label = node.label;
    if (includeVectors && node.vector) nxNode.vector = node.vector;
    if (includeMetadata && node.attributes) Object.assign(nxNode, node.attributes);

    return nxNode;
  });

  const links = graph.edges.map(edge => ({
    source: edge.source,
    target: edge.target,
    weight: edge.weight,
    type: edge.type || 'similarity',
    ...(edge.attributes || {})
  }));

  return {
    directed: true,
    multigraph: false,
    graph: graph.metadata || {},
    nodes,
    links
  };
}

/**
 * Export graph to NetworkX edge list format
 *
 * Creates a simple text format with one edge per line.
 * Format: source target weight
 *
 * @param graph - Graph to export
 * @returns Edge list string
 */
export function exportToNetworkXEdgeList(graph: Graph): string {
  let edgeList = '# Source Target Weight\n';

  for (const edge of graph.edges) {
    edgeList += `${edge.source} ${edge.target} ${edge.weight}\n`;
  }

  return edgeList;
}

/**
 * Export graph to NetworkX adjacency list format
 *
 * @param graph - Graph to export
 * @returns Adjacency list string
 */
export function exportToNetworkXAdjacencyList(graph: Graph): string {
  const adjacency = new Map<string, Array<{ target: string; weight: number }>>();

  // Build adjacency structure
  for (const edge of graph.edges) {
    if (!adjacency.has(edge.source)) {
      adjacency.set(edge.source, []);
    }
    adjacency.get(edge.source)!.push({
      target: edge.target,
      weight: edge.weight
    });
  }

  let adjList = '# Adjacency List\n';
  Array.from(adjacency.entries()).forEach(([source, neighbors]) => {
    const neighborStr = neighbors
      .map(n => `${n.target}:${n.weight}`)
      .join(' ');
    adjList += `${source} ${neighborStr}\n`;
  });

  return adjList;
}

// ============================================================================
// Unified Export Function
// ============================================================================

/**
 * Export graph to specified format
 *
 * Universal export function that routes to the appropriate format exporter.
 *
 * @param graph - Graph to export
 * @param format - Target export format
 * @param options - Export options
 * @returns Export result with data and metadata
 *
 * @example
 * ```typescript
 * // Export to GraphML
 * const result = exportGraph(graph, 'graphml', {
 *   graphName: 'My Graph',
 *   includeVectors: false
 * });
 * console.log(result.data);
 *
 * // Export to D3.js
 * const d3Result = exportGraph(graph, 'd3');
 * // d3Result.data is a JSON object
 * ```
 */
export function exportGraph(
  graph: Graph,
  format: ExportFormat,
  options: ExportOptions = {}
): ExportResult {
  let data: string | object;

  switch (format) {
    case 'graphml':
      data = exportToGraphML(graph, options);
      break;

    case 'gexf':
      data = exportToGEXF(graph, options);
      break;

    case 'neo4j':
      data = exportToNeo4j(graph, options);
      break;

    case 'd3':
      data = exportToD3(graph, options);
      break;

    case 'networkx':
      data = exportToNetworkX(graph, options);
      break;

    default:
      throw new Error(`Unsupported export format: ${format}`);
  }

  return {
    format,
    data,
    nodeCount: graph.nodes.length,
    edgeCount: graph.edges.length,
    metadata: {
      timestamp: new Date().toISOString(),
      options,
      ...graph.metadata
    }
  };
}

// ============================================================================
// Streaming Exporters
// ============================================================================

/**
 * Base class for streaming graph exporters
 */
export abstract class StreamingExporter {
  protected stream: Writable;
  protected options: ExportOptions;

  constructor(stream: Writable, options: ExportOptions = {}) {
    this.stream = stream;
    this.options = options;
  }

  protected write(data: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.stream.write(data, (err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  }

  abstract start(): Promise<void>;
  abstract addNode(node: GraphNode): Promise<void>;
  abstract addEdge(edge: GraphEdge): Promise<void>;
  abstract end(): Promise<void>;
}

/**
 * Streaming GraphML exporter
 *
 * @example
 * ```typescript
 * const stream = createWriteStream('graph.graphml');
 * const exporter = new GraphMLStreamExporter(stream);
 *
 * await exporter.start();
 * for (const node of nodes) {
 *   await exporter.addNode(node);
 * }
 * for (const edge of edges) {
 *   await exporter.addEdge(edge);
 * }
 * await exporter.end();
 * ```
 */
export class GraphMLStreamExporter extends StreamingExporter {
  private nodeAttributesDefined = false;

  async start(): Promise<void> {
    let xml = '<?xml version="1.0" encoding="UTF-8"?>\n';
    xml += '<graphml xmlns="http://graphml.graphdrawing.org/xmlns"\n';
    xml += '         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"\n';
    xml += '         xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns\n';
    xml += '         http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">\n\n';
    xml += '  <key id="label" for="node" attr.name="label" attr.type="string"/>\n';
    xml += '  <key id="weight" for="edge" attr.name="weight" attr.type="double"/>\n\n';
    xml += `  <graph id="${escapeXML(this.options.graphName || 'VectorGraph')}" edgedefault="directed">\n\n`;

    await this.write(xml);
  }

  async addNode(node: GraphNode): Promise<void> {
    let xml = `    <node id="${escapeXML(node.id)}">\n`;

    if (node.label) {
      xml += `      <data key="label">${escapeXML(node.label)}</data>\n`;
    }

    xml += '    </node>\n';
    await this.write(xml);
  }

  async addEdge(edge: GraphEdge): Promise<void> {
    const edgeId = `e_${edge.source}_${edge.target}`;
    let xml = `    <edge id="${escapeXML(edgeId)}" source="${escapeXML(edge.source)}" target="${escapeXML(edge.target)}">\n`;
    xml += `      <data key="weight">${edge.weight}</data>\n`;
    xml += '    </edge>\n';
    await this.write(xml);
  }

  async end(): Promise<void> {
    const xml = '  </graph>\n</graphml>\n';
    await this.write(xml);
  }
}

/**
 * Streaming D3.js JSON exporter
 */
export class D3StreamExporter extends StreamingExporter {
  private firstNode = true;
  private firstEdge = true;
  private nodePhase = true;

  async start(): Promise<void> {
    await this.write('{"nodes":[');
  }

  async addNode(node: GraphNode): Promise<void> {
    if (!this.nodePhase) {
      throw new Error('Cannot add nodes after edges have been added');
    }

    const d3Node: any = {
      id: node.id,
      name: node.label || node.id,
      ...node.attributes
    };

    const prefix = this.firstNode ? '' : ',';
    this.firstNode = false;

    await this.write(prefix + JSON.stringify(d3Node));
  }

  async addEdge(edge: GraphEdge): Promise<void> {
    if (this.nodePhase) {
      this.nodePhase = false;
      await this.write('],"links":[');
    }

    const d3Link = {
      source: edge.source,
      target: edge.target,
      value: edge.weight,
      type: edge.type || 'similarity'
    };

    const prefix = this.firstEdge ? '' : ',';
    this.firstEdge = false;

    await this.write(prefix + JSON.stringify(d3Link));
  }

  async end(): Promise<void> {
    if (this.nodePhase) {
      await this.write('],"links":[');
    }
    await this.write(']}');
  }
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Escape XML special characters
 */
function escapeXML(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

/**
 * Escape Cypher special characters
 */
function escapeCypher(str: string): string {
  return str.replace(/"/g, '\\"').replace(/\\/g, '\\\\');
}

/**
 * Validate graph structure
 *
 * @param graph - Graph to validate
 * @throws Error if graph is invalid
 */
export function validateGraph(graph: Graph): void {
  if (!graph.nodes || !Array.isArray(graph.nodes)) {
    throw new Error('Graph must have a nodes array');
  }

  if (!graph.edges || !Array.isArray(graph.edges)) {
    throw new Error('Graph must have an edges array');
  }

  const nodeIds = new Set(graph.nodes.map(n => n.id));

  for (const node of graph.nodes) {
    if (!node.id) {
      throw new Error('All nodes must have an id');
    }
  }

  for (const edge of graph.edges) {
    if (!edge.source || !edge.target) {
      throw new Error('All edges must have source and target');
    }

    if (!nodeIds.has(edge.source)) {
      throw new Error(`Edge references non-existent source node: ${edge.source}`);
    }

    if (!nodeIds.has(edge.target)) {
      throw new Error(`Edge references non-existent target node: ${edge.target}`);
    }

    if (typeof edge.weight !== 'number') {
      throw new Error('All edges must have a numeric weight');
    }
  }
}

// ============================================================================
// Exports
// ============================================================================

export default {
  // Graph builders
  buildGraphFromEntries,
  buildGraphFromVectorDB,

  // Format exporters
  exportToGraphML,
  exportToGEXF,
  exportToNeo4j,
  exportToNeo4jJSON,
  exportToD3,
  exportToD3Hierarchy,
  exportToNetworkX,
  exportToNetworkXEdgeList,
  exportToNetworkXAdjacencyList,

  // Unified export
  exportGraph,

  // Streaming exporters
  GraphMLStreamExporter,
  D3StreamExporter,
  streamToGraphML,

  // Utilities
  validateGraph,
  cosineSimilarity
};
