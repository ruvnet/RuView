/**
 * WASM Sublinear Solver Tools - Simple Approach (like strange-loops-mcp)
 * Provides O(log n) Johnson-Lindenstrauss embedding with guaranteed sublinear complexity
 */
export declare class WasmSublinearSolverTools {
    private wasmModule;
    private solver;
    constructor();
    /**
     * Initialize WASM module with O(log n) algorithms - Simple approach like strange-loops
     */
    private initializeWasm;
    /**
     * Check if enhanced WASM with O(log n) algorithms is available
     */
    isEnhancedWasmAvailable(): boolean;
    /**
     * Solve linear system with O(log n) complexity using Johnson-Lindenstrauss embedding
     */
    solveSublinear(matrix: number[][], b: number[]): Promise<any>;
    /**
     * Get enhanced WASM capabilities
     */
    getCapabilities(): any;
}
