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
import type { VectorEntry } from 'ruvector';
type VectorDBInstance = any;
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
export declare function buildGraphFromVectorDB(db: VectorDBInstance, options?: ExportOptions): Graph;
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
export declare function buildGraphFromEntries(entries: VectorEntry[], options?: ExportOptions): Graph;
/**
 * Compute cosine similarity between two vectors
 */
declare function cosineSimilarity(a: number[], b: number[]): number;
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
export declare function exportToGraphML(graph: Graph, options?: ExportOptions): string;
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
export declare function streamToGraphML(graph: Graph, stream: Writable, options?: ExportOptions): Promise<void>;
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
export declare function exportToGEXF(graph: Graph, options?: ExportOptions): string;
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
export declare function exportToNeo4j(graph: Graph, options?: ExportOptions): string;
/**
 * Export graph to Neo4j JSON format (for neo4j-admin import)
 *
 * @param graph - Graph to export
 * @param options - Export options
 * @returns Neo4j JSON import format
 */
export declare function exportToNeo4jJSON(graph: Graph, options?: ExportOptions): {
    nodes: any[];
    relationships: any[];
};
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
export declare function exportToD3(graph: Graph, options?: ExportOptions): {
    nodes: any[];
    links: any[];
};
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
export declare function exportToD3Hierarchy(graph: Graph, rootId: string, options?: ExportOptions): any;
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
export declare function exportToNetworkX(graph: Graph, options?: ExportOptions): any;
/**
 * Export graph to NetworkX edge list format
 *
 * Creates a simple text format with one edge per line.
 * Format: source target weight
 *
 * @param graph - Graph to export
 * @returns Edge list string
 */
export declare function exportToNetworkXEdgeList(graph: Graph): string;
/**
 * Export graph to NetworkX adjacency list format
 *
 * @param graph - Graph to export
 * @returns Adjacency list string
 */
export declare function exportToNetworkXAdjacencyList(graph: Graph): string;
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
export declare function exportGraph(graph: Graph, format: ExportFormat, options?: ExportOptions): ExportResult;
/**
 * Base class for streaming graph exporters
 */
export declare abstract class StreamingExporter {
    protected stream: Writable;
    protected options: ExportOptions;
    constructor(stream: Writable, options?: ExportOptions);
    protected write(data: string): Promise<void>;
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
export declare class GraphMLStreamExporter extends StreamingExporter {
    private nodeAttributesDefined;
    start(): Promise<void>;
    addNode(node: GraphNode): Promise<void>;
    addEdge(edge: GraphEdge): Promise<void>;
    end(): Promise<void>;
}
/**
 * Streaming D3.js JSON exporter
 */
export declare class D3StreamExporter extends StreamingExporter {
    private firstNode;
    private firstEdge;
    private nodePhase;
    start(): Promise<void>;
    addNode(node: GraphNode): Promise<void>;
    addEdge(edge: GraphEdge): Promise<void>;
    end(): Promise<void>;
}
/**
 * Validate graph structure
 *
 * @param graph - Graph to validate
 * @throws Error if graph is invalid
 */
export declare function validateGraph(graph: Graph): void;
declare const _default: {
    buildGraphFromEntries: typeof buildGraphFromEntries;
    buildGraphFromVectorDB: typeof buildGraphFromVectorDB;
    exportToGraphML: typeof exportToGraphML;
    exportToGEXF: typeof exportToGEXF;
    exportToNeo4j: typeof exportToNeo4j;
    exportToNeo4jJSON: typeof exportToNeo4jJSON;
    exportToD3: typeof exportToD3;
    exportToD3Hierarchy: typeof exportToD3Hierarchy;
    exportToNetworkX: typeof exportToNetworkX;
    exportToNetworkXEdgeList: typeof exportToNetworkXEdgeList;
    exportToNetworkXAdjacencyList: typeof exportToNetworkXAdjacencyList;
    exportGraph: typeof exportGraph;
    GraphMLStreamExporter: typeof GraphMLStreamExporter;
    D3StreamExporter: typeof D3StreamExporter;
    streamToGraphML: typeof streamToGraphML;
    validateGraph: typeof validateGraph;
    cosineSimilarity: typeof cosineSimilarity;
};
export default _default;
//# sourceMappingURL=exporters.d.ts.map