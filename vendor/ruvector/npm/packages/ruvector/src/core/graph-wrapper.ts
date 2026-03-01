/**
 * Graph Wrapper - Hypergraph database for code relationships
 *
 * Wraps @ruvector/graph-node for dependency analysis, co-edit patterns,
 * and code structure understanding.
 */

let graphModule: any = null;
let loadError: Error | null = null;

function getGraphModule() {
  if (graphModule) return graphModule;
  if (loadError) throw loadError;

  try {
    graphModule = require('@ruvector/graph-node');
    return graphModule;
  } catch (e: any) {
    loadError = new Error(
      `@ruvector/graph-node not installed: ${e.message}\n` +
      `Install with: npm install @ruvector/graph-node`
    );
    throw loadError;
  }
}

export function isGraphAvailable(): boolean {
  try {
    getGraphModule();
    return true;
  } catch {
    return false;
  }
}

export interface Node {
  id: string;
  labels: string[];
  properties: Record<string, any>;
}

export interface Edge {
  id?: string;
  from: string;
  to: string;
  type: string;
  properties?: Record<string, any>;
}

export interface Hyperedge {
  id?: string;
  nodes: string[];
  type: string;
  properties?: Record<string, any>;
}

export interface CypherResult {
  columns: string[];
  rows: any[][];
}

export interface PathResult {
  nodes: Node[];
  edges: Edge[];
  length: number;
}

/**
 * Graph Database for code relationships
 */
export class CodeGraph {
  private inner: any;
  private storagePath?: string;

  constructor(options: { storagePath?: string; inMemory?: boolean } = {}) {
    const graph = getGraphModule();
    this.storagePath = options.storagePath;
    this.inner = new graph.GraphDatabase({
      storagePath: options.storagePath,
      inMemory: options.inMemory ?? true,
    });
  }

  // ===========================================================================
  // Node Operations
  // ===========================================================================

  /**
   * Create a node (file, function, class, etc.)
   */
  createNode(id: string, labels: string[], properties: Record<string, any> = {}): Node {
    this.inner.createNode(id, labels, JSON.stringify(properties));
    return { id, labels, properties };
  }

  /**
   * Get a node by ID
   */
  getNode(id: string): Node | null {
    const result = this.inner.getNode(id);
    if (!result) return null;
    return {
      id: result.id,
      labels: result.labels,
      properties: result.properties ? JSON.parse(result.properties) : {},
    };
  }

  /**
   * Update node properties
   */
  updateNode(id: string, properties: Record<string, any>): boolean {
    return this.inner.updateNode(id, JSON.stringify(properties));
  }

  /**
   * Delete a node
   */
  deleteNode(id: string): boolean {
    return this.inner.deleteNode(id);
  }

  /**
   * Find nodes by label
   */
  findNodesByLabel(label: string): Node[] {
    const results = this.inner.findNodesByLabel(label);
    return results.map((r: any) => ({
      id: r.id,
      labels: r.labels,
      properties: r.properties ? JSON.parse(r.properties) : {},
    }));
  }

  // ===========================================================================
  // Edge Operations
  // ===========================================================================

  /**
   * Create an edge (import, call, reference, etc.)
   */
  createEdge(from: string, to: string, type: string, properties: Record<string, any> = {}): Edge {
    const id = this.inner.createEdge(from, to, type, JSON.stringify(properties));
    return { id, from, to, type, properties };
  }

  /**
   * Get edges from a node
   */
  getOutgoingEdges(nodeId: string, type?: string): Edge[] {
    const results = this.inner.getOutgoingEdges(nodeId, type);
    return results.map((r: any) => ({
      id: r.id,
      from: r.from,
      to: r.to,
      type: r.type,
      properties: r.properties ? JSON.parse(r.properties) : {},
    }));
  }

  /**
   * Get edges to a node
   */
  getIncomingEdges(nodeId: string, type?: string): Edge[] {
    const results = this.inner.getIncomingEdges(nodeId, type);
    return results.map((r: any) => ({
      id: r.id,
      from: r.from,
      to: r.to,
      type: r.type,
      properties: r.properties ? JSON.parse(r.properties) : {},
    }));
  }

  /**
   * Delete an edge
   */
  deleteEdge(edgeId: string): boolean {
    return this.inner.deleteEdge(edgeId);
  }

  // ===========================================================================
  // Hyperedge Operations (for co-edit patterns)
  // ===========================================================================

  /**
   * Create a hyperedge connecting multiple nodes
   */
  createHyperedge(nodes: string[], type: string, properties: Record<string, any> = {}): Hyperedge {
    const id = this.inner.createHyperedge(nodes, type, JSON.stringify(properties));
    return { id, nodes, type, properties };
  }

  /**
   * Get hyperedges containing a node
   */
  getHyperedges(nodeId: string, type?: string): Hyperedge[] {
    const results = this.inner.getHyperedges(nodeId, type);
    return results.map((r: any) => ({
      id: r.id,
      nodes: r.nodes,
      type: r.type,
      properties: r.properties ? JSON.parse(r.properties) : {},
    }));
  }

  // ===========================================================================
  // Query Operations
  // ===========================================================================

  /**
   * Execute a Cypher query
   */
  cypher(query: string, params: Record<string, any> = {}): CypherResult {
    const result = this.inner.cypher(query, JSON.stringify(params));
    return {
      columns: result.columns,
      rows: result.rows,
    };
  }

  /**
   * Find shortest path between nodes
   */
  shortestPath(from: string, to: string, maxDepth: number = 10): PathResult | null {
    const result = this.inner.shortestPath(from, to, maxDepth);
    if (!result) return null;
    return {
      nodes: result.nodes.map((n: any) => ({
        id: n.id,
        labels: n.labels,
        properties: n.properties ? JSON.parse(n.properties) : {},
      })),
      edges: result.edges.map((e: any) => ({
        id: e.id,
        from: e.from,
        to: e.to,
        type: e.type,
        properties: e.properties ? JSON.parse(e.properties) : {},
      })),
      length: result.length,
    };
  }

  /**
   * Get all paths between nodes (up to maxPaths)
   */
  allPaths(from: string, to: string, maxDepth: number = 5, maxPaths: number = 10): PathResult[] {
    const results = this.inner.allPaths(from, to, maxDepth, maxPaths);
    return results.map((r: any) => ({
      nodes: r.nodes.map((n: any) => ({
        id: n.id,
        labels: n.labels,
        properties: n.properties ? JSON.parse(n.properties) : {},
      })),
      edges: r.edges.map((e: any) => ({
        id: e.id,
        from: e.from,
        to: e.to,
        type: e.type,
        properties: e.properties ? JSON.parse(e.properties) : {},
      })),
      length: r.length,
    }));
  }

  /**
   * Get neighbors of a node
   */
  neighbors(nodeId: string, depth: number = 1): Node[] {
    const results = this.inner.neighbors(nodeId, depth);
    return results.map((n: any) => ({
      id: n.id,
      labels: n.labels,
      properties: n.properties ? JSON.parse(n.properties) : {},
    }));
  }

  // ===========================================================================
  // Graph Algorithms
  // ===========================================================================

  /**
   * Calculate PageRank for nodes
   */
  pageRank(iterations: number = 20, dampingFactor: number = 0.85): Map<string, number> {
    const result = this.inner.pageRank(iterations, dampingFactor);
    return new Map(Object.entries(result));
  }

  /**
   * Find connected components
   */
  connectedComponents(): string[][] {
    return this.inner.connectedComponents();
  }

  /**
   * Detect communities (Louvain algorithm)
   */
  communities(): Map<string, number> {
    const result = this.inner.communities();
    return new Map(Object.entries(result));
  }

  /**
   * Calculate betweenness centrality
   */
  betweennessCentrality(): Map<string, number> {
    const result = this.inner.betweennessCentrality();
    return new Map(Object.entries(result));
  }

  // ===========================================================================
  // Persistence
  // ===========================================================================

  /**
   * Save graph to storage
   */
  save(): void {
    if (!this.storagePath) {
      throw new Error('No storage path configured');
    }
    this.inner.save();
  }

  /**
   * Load graph from storage
   */
  load(): void {
    if (!this.storagePath) {
      throw new Error('No storage path configured');
    }
    this.inner.load();
  }

  /**
   * Clear all data
   */
  clear(): void {
    this.inner.clear();
  }

  /**
   * Get graph statistics
   */
  stats(): { nodes: number; edges: number; hyperedges: number } {
    return this.inner.stats();
  }
}

/**
 * Create a code dependency graph from file analysis
 */
export function createCodeDependencyGraph(storagePath?: string): CodeGraph {
  return new CodeGraph({ storagePath, inMemory: !storagePath });
}

export default CodeGraph;
