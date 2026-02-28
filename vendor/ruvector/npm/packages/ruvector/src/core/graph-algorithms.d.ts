/**
 * Graph Algorithms - MinCut, Spectral Clustering, Community Detection
 *
 * Provides graph partitioning and clustering algorithms for:
 * - Code module detection
 * - Dependency clustering
 * - Architecture analysis
 * - Refactoring suggestions
 */
export interface Graph {
    nodes: string[];
    edges: Array<{
        from: string;
        to: string;
        weight?: number;
    }>;
    adjacency: Map<string, Map<string, number>>;
}
export interface Partition {
    groups: string[][];
    cutWeight: number;
    modularity: number;
}
export interface SpectralResult {
    clusters: Map<string, number>;
    eigenvalues: number[];
    coordinates: Map<string, number[]>;
}
/**
 * Build adjacency representation from edges
 */
export declare function buildGraph(nodes: string[], edges: Array<{
    from: string;
    to: string;
    weight?: number;
}>): Graph;
/**
 * Minimum Cut (Stoer-Wagner algorithm)
 *
 * Finds the minimum weight cut that partitions the graph into two parts.
 * Useful for finding loosely coupled module boundaries.
 */
export declare function minCut(graph: Graph): Partition;
/**
 * Spectral Clustering (using power iteration)
 *
 * Uses graph Laplacian eigenvectors for clustering.
 * Good for finding natural clusters in code dependencies.
 */
export declare function spectralClustering(graph: Graph, k?: number): SpectralResult;
/**
 * Louvain Community Detection
 *
 * Greedy modularity optimization for finding communities.
 * Good for detecting natural module boundaries.
 */
export declare function louvainCommunities(graph: Graph): Map<string, number>;
/**
 * Calculate modularity of a partition
 */
export declare function calculateModularity(graph: Graph, partition: string[][]): number;
/**
 * Find bridges (edges whose removal disconnects components)
 */
export declare function findBridges(graph: Graph): Array<{
    from: string;
    to: string;
}>;
/**
 * Find articulation points (nodes whose removal disconnects components)
 */
export declare function findArticulationPoints(graph: Graph): string[];
declare const _default: {
    buildGraph: typeof buildGraph;
    minCut: typeof minCut;
    spectralClustering: typeof spectralClustering;
    louvainCommunities: typeof louvainCommunities;
    calculateModularity: typeof calculateModularity;
    findBridges: typeof findBridges;
    findArticulationPoints: typeof findArticulationPoints;
};
export default _default;
//# sourceMappingURL=graph-algorithms.d.ts.map