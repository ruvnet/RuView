/**
 * Complete WASM Sublinear Solver - All 4 Algorithms from Plans
 *
 * Implements:
 * - Neumann Series: O(k·nnz)
 * - Forward Push: O(1/ε) for single query
 * - Backward Push: O(1/ε) for single query
 * - Hybrid Random-Walk: O(√n/ε)
 * - Method Auto-Selection
 */
interface SolverConfig {
    method?: 'auto' | 'neumann' | 'forward-push' | 'backward-push' | 'random-walk';
    epsilon?: number;
    maxIterations?: number;
    precision?: 'single' | 'double' | 'adaptive';
    targetIndex?: number;
    sourceIndex?: number;
    precision_requirement?: number;
}
export declare class CompleteWasmSublinearSolverTools {
    private wasmModule;
    private solver;
    constructor();
    /**
     * Initialize WASM module with complete sublinear algorithms
     */
    private initializeWasm;
    /**
     * Check if complete WASM is available
     */
    isCompleteWasmAvailable(): boolean;
    /**
     * Solve with complete algorithm suite and auto-selection
     */
    solveComplete(matrix: number[][], b: number[], config?: SolverConfig): Promise<any>;
    /**
     * Get complete solver capabilities
     */
    getCompleteCapabilities(): any;
}
export {};
