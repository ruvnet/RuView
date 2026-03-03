/**
 * PageRank integration with O(log n) WASM solver
 */
export declare class PageRankTools {
    private static wasmSolver;
    /**
     * Get or create WASM solver instance
     */
    private static getWasmSolver;
    /**
     * Compute PageRank with O(log n) complexity using enhanced WASM
     */
    static pageRank(params: {
        adjacency: any;
        damping?: number;
        personalized?: number[];
    }): Promise<{
        pageRankVector: any;
        topNodes: any;
        totalScore: any;
        maxScore: number;
        minScore: number;
        complexity_bound: any;
        compression_ratio: any;
        algorithm: any;
        mathematical_guarantee: any;
        metadata: any;
    }>;
    /**
     * Get PageRank capabilities
     */
    static getCapabilities(): any;
}
export default PageRankTools;
