/**
 * MCP Tools for graph algorithms using sublinear solvers
 */
import { Matrix, Vector, PageRankParams, EffectiveResistanceParams } from '../../core/types.js';
export declare class GraphTools {
    /**
     * Compute PageRank using sublinear solver
     */
    static pageRank(params: PageRankParams): Promise<{
        pageRankVector: Vector;
        topNodes: {
            node: number;
            score: number;
        }[];
        bottomNodes: {
            node: number;
            score: number;
        }[];
        statistics: {
            totalScore: number;
            maxScore: number;
            minScore: number;
            mean: number;
            standardDeviation: number;
            entropy: number;
            convergenceInfo: {
                damping: number;
                personalized: boolean;
            };
        };
        distribution: {
            quantiles: Record<string, number>;
            concentrationRatio: number;
        };
    }>;
    /**
     * Compute personalized PageRank for specific nodes
     */
    static personalizedPageRank(adjacency: Matrix, personalizeNodes: number[], params?: Partial<PageRankParams>): Promise<{
        personalizedFor: number[];
        influence: {
            directInfluence: number[];
            totalInfluence: number;
        };
        pageRankVector: Vector;
        topNodes: {
            node: number;
            score: number;
        }[];
        bottomNodes: {
            node: number;
            score: number;
        }[];
        statistics: {
            totalScore: number;
            maxScore: number;
            minScore: number;
            mean: number;
            standardDeviation: number;
            entropy: number;
            convergenceInfo: {
                damping: number;
                personalized: boolean;
            };
        };
        distribution: {
            quantiles: Record<string, number>;
            concentrationRatio: number;
        };
    }>;
    /**
     * Compute effective resistance between nodes
     */
    static effectiveResistance(params: EffectiveResistanceParams): Promise<{
        effectiveResistance: number;
        voltage: number[];
        source: number;
        target: number;
        convergenceInfo: {
            iterations: number;
            residual: number;
            converged: boolean;
        };
    }>;
    /**
     * Compute centrality measures using sublinear methods
     */
    static computeCentralities(adjacency: Matrix, measures?: string[]): Promise<Record<string, any>>;
    /**
     * Detect communities using spectral methods
     */
    static detectCommunities(adjacency: Matrix, numCommunities?: number): Promise<{
        communities: number[][];
        assignments: any[];
        modularity: number;
        quality: {
            numCommunities: number;
            largestCommunity: number;
            smallestCommunity: number;
        };
    }>;
    private static computeQuantiles;
    private static createGroundedLaplacian;
    private static createNormalizedLaplacian;
    private static closenessCentrality;
    private static betweennessCentrality;
    private static computeModularity;
    private static countEdges;
    private static getNodeDegree;
}
