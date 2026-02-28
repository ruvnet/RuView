/**
 * Graph Wrapper - Hypergraph database for code relationships
 *
 * Wraps @ruvector/graph-node for dependency analysis, co-edit patterns,
 * and code structure understanding.
 */
export declare function isGraphAvailable(): boolean;
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
export declare class CodeGraph {
    private inner;
    private storagePath?;
    constructor(options?: {
        storagePath?: string;
        inMemory?: boolean;
    });
    /**
     * Create a node (file, function, class, etc.)
     */
    createNode(id: string, labels: string[], properties?: Record<string, any>): Node;
    /**
     * Get a node by ID
     */
    getNode(id: string): Node | null;
    /**
     * Update node properties
     */
    updateNode(id: string, properties: Record<string, any>): boolean;
    /**
     * Delete a node
     */
    deleteNode(id: string): boolean;
    /**
     * Find nodes by label
     */
    findNodesByLabel(label: string): Node[];
    /**
     * Create an edge (import, call, reference, etc.)
     */
    createEdge(from: string, to: string, type: string, properties?: Record<string, any>): Edge;
    /**
     * Get edges from a node
     */
    getOutgoingEdges(nodeId: string, type?: string): Edge[];
    /**
     * Get edges to a node
     */
    getIncomingEdges(nodeId: string, type?: string): Edge[];
    /**
     * Delete an edge
     */
    deleteEdge(edgeId: string): boolean;
    /**
     * Create a hyperedge connecting multiple nodes
     */
    createHyperedge(nodes: string[], type: string, properties?: Record<string, any>): Hyperedge;
    /**
     * Get hyperedges containing a node
     */
    getHyperedges(nodeId: string, type?: string): Hyperedge[];
    /**
     * Execute a Cypher query
     */
    cypher(query: string, params?: Record<string, any>): CypherResult;
    /**
     * Find shortest path between nodes
     */
    shortestPath(from: string, to: string, maxDepth?: number): PathResult | null;
    /**
     * Get all paths between nodes (up to maxPaths)
     */
    allPaths(from: string, to: string, maxDepth?: number, maxPaths?: number): PathResult[];
    /**
     * Get neighbors of a node
     */
    neighbors(nodeId: string, depth?: number): Node[];
    /**
     * Calculate PageRank for nodes
     */
    pageRank(iterations?: number, dampingFactor?: number): Map<string, number>;
    /**
     * Find connected components
     */
    connectedComponents(): string[][];
    /**
     * Detect communities (Louvain algorithm)
     */
    communities(): Map<string, number>;
    /**
     * Calculate betweenness centrality
     */
    betweennessCentrality(): Map<string, number>;
    /**
     * Save graph to storage
     */
    save(): void;
    /**
     * Load graph from storage
     */
    load(): void;
    /**
     * Clear all data
     */
    clear(): void;
    /**
     * Get graph statistics
     */
    stats(): {
        nodes: number;
        edges: number;
        hyperedges: number;
    };
}
/**
 * Create a code dependency graph from file analysis
 */
export declare function createCodeDependencyGraph(storagePath?: string): CodeGraph;
export default CodeGraph;
//# sourceMappingURL=graph-wrapper.d.ts.map