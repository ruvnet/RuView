/**
 * WASM-based O(log n) Sublinear Solver for MCP Tools
 *
 * This integrates our enhanced WASM with Johnson-Lindenstrauss embedding
 * to provide true O(log n) complexity for the MCP server
 */
export declare class WasmSublinearSolverTools {
    private wasmModule;
    private solver;
    constructor();
    /**
     * Initialize WASM module with O(log n) algorithms
     */
    private initializeWasm;
    /**
     * Solve linear system with O(log n) complexity using Johnson-Lindenstrauss embedding
     */
    solveSublinear(matrix: number[][], b: number[]): Promise<any>;
    /**
     * Compute PageRank with O(log n) complexity
     */
    pageRankSublinear(adjacency: number[][], damping?: number, personalized?: number[]): Promise<any>;
    /**
     * Check if O(log n) WASM is available
     */
    isEnhancedWasmAvailable(): boolean;
    /**
     * Get solver capabilities and complexity bounds
     */
    getCapabilities(): any;
    /**
     * Clean up WASM resources
     */
    dispose(): void;
}
export default WasmSublinearSolverTools;
