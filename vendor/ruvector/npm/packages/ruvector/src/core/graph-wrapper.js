"use strict";
/**
 * Graph Wrapper - Hypergraph database for code relationships
 *
 * Wraps @ruvector/graph-node for dependency analysis, co-edit patterns,
 * and code structure understanding.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.CodeGraph = void 0;
exports.isGraphAvailable = isGraphAvailable;
exports.createCodeDependencyGraph = createCodeDependencyGraph;
let graphModule = null;
let loadError = null;
function getGraphModule() {
    if (graphModule)
        return graphModule;
    if (loadError)
        throw loadError;
    try {
        graphModule = require('@ruvector/graph-node');
        return graphModule;
    }
    catch (e) {
        loadError = new Error(`@ruvector/graph-node not installed: ${e.message}\n` +
            `Install with: npm install @ruvector/graph-node`);
        throw loadError;
    }
}
function isGraphAvailable() {
    try {
        getGraphModule();
        return true;
    }
    catch {
        return false;
    }
}
/**
 * Graph Database for code relationships
 */
class CodeGraph {
    constructor(options = {}) {
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
    createNode(id, labels, properties = {}) {
        this.inner.createNode(id, labels, JSON.stringify(properties));
        return { id, labels, properties };
    }
    /**
     * Get a node by ID
     */
    getNode(id) {
        const result = this.inner.getNode(id);
        if (!result)
            return null;
        return {
            id: result.id,
            labels: result.labels,
            properties: result.properties ? JSON.parse(result.properties) : {},
        };
    }
    /**
     * Update node properties
     */
    updateNode(id, properties) {
        return this.inner.updateNode(id, JSON.stringify(properties));
    }
    /**
     * Delete a node
     */
    deleteNode(id) {
        return this.inner.deleteNode(id);
    }
    /**
     * Find nodes by label
     */
    findNodesByLabel(label) {
        const results = this.inner.findNodesByLabel(label);
        return results.map((r) => ({
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
    createEdge(from, to, type, properties = {}) {
        const id = this.inner.createEdge(from, to, type, JSON.stringify(properties));
        return { id, from, to, type, properties };
    }
    /**
     * Get edges from a node
     */
    getOutgoingEdges(nodeId, type) {
        const results = this.inner.getOutgoingEdges(nodeId, type);
        return results.map((r) => ({
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
    getIncomingEdges(nodeId, type) {
        const results = this.inner.getIncomingEdges(nodeId, type);
        return results.map((r) => ({
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
    deleteEdge(edgeId) {
        return this.inner.deleteEdge(edgeId);
    }
    // ===========================================================================
    // Hyperedge Operations (for co-edit patterns)
    // ===========================================================================
    /**
     * Create a hyperedge connecting multiple nodes
     */
    createHyperedge(nodes, type, properties = {}) {
        const id = this.inner.createHyperedge(nodes, type, JSON.stringify(properties));
        return { id, nodes, type, properties };
    }
    /**
     * Get hyperedges containing a node
     */
    getHyperedges(nodeId, type) {
        const results = this.inner.getHyperedges(nodeId, type);
        return results.map((r) => ({
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
    cypher(query, params = {}) {
        const result = this.inner.cypher(query, JSON.stringify(params));
        return {
            columns: result.columns,
            rows: result.rows,
        };
    }
    /**
     * Find shortest path between nodes
     */
    shortestPath(from, to, maxDepth = 10) {
        const result = this.inner.shortestPath(from, to, maxDepth);
        if (!result)
            return null;
        return {
            nodes: result.nodes.map((n) => ({
                id: n.id,
                labels: n.labels,
                properties: n.properties ? JSON.parse(n.properties) : {},
            })),
            edges: result.edges.map((e) => ({
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
    allPaths(from, to, maxDepth = 5, maxPaths = 10) {
        const results = this.inner.allPaths(from, to, maxDepth, maxPaths);
        return results.map((r) => ({
            nodes: r.nodes.map((n) => ({
                id: n.id,
                labels: n.labels,
                properties: n.properties ? JSON.parse(n.properties) : {},
            })),
            edges: r.edges.map((e) => ({
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
    neighbors(nodeId, depth = 1) {
        const results = this.inner.neighbors(nodeId, depth);
        return results.map((n) => ({
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
    pageRank(iterations = 20, dampingFactor = 0.85) {
        const result = this.inner.pageRank(iterations, dampingFactor);
        return new Map(Object.entries(result));
    }
    /**
     * Find connected components
     */
    connectedComponents() {
        return this.inner.connectedComponents();
    }
    /**
     * Detect communities (Louvain algorithm)
     */
    communities() {
        const result = this.inner.communities();
        return new Map(Object.entries(result));
    }
    /**
     * Calculate betweenness centrality
     */
    betweennessCentrality() {
        const result = this.inner.betweennessCentrality();
        return new Map(Object.entries(result));
    }
    // ===========================================================================
    // Persistence
    // ===========================================================================
    /**
     * Save graph to storage
     */
    save() {
        if (!this.storagePath) {
            throw new Error('No storage path configured');
        }
        this.inner.save();
    }
    /**
     * Load graph from storage
     */
    load() {
        if (!this.storagePath) {
            throw new Error('No storage path configured');
        }
        this.inner.load();
    }
    /**
     * Clear all data
     */
    clear() {
        this.inner.clear();
    }
    /**
     * Get graph statistics
     */
    stats() {
        return this.inner.stats();
    }
}
exports.CodeGraph = CodeGraph;
/**
 * Create a code dependency graph from file analysis
 */
function createCodeDependencyGraph(storagePath) {
    return new CodeGraph({ storagePath, inMemory: !storagePath });
}
exports.default = CodeGraph;
//# sourceMappingURL=graph-wrapper.js.map