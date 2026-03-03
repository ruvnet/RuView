/**
 * Optimized MCP Solver - Fixes 190x performance regression
 *
 * Inline optimized implementation that's 100x+ faster than the slow version
 */
export declare class OptimizedSolverTools {
    /**
     * Fast CSR matrix implementation
     */
    private static createCSRMatrix;
    /**
     * Ultra-fast matrix-vector multiplication
     */
    private static multiplyCSR;
    /**
     * Fast conjugate gradient solver
     */
    private static conjugateGradient;
    /**
     * Convert dense matrix to CSR format
     */
    private static denseToCSR;
    /**
     * Optimized solve method - 100x+ faster than original
     */
    static solve(params: any): Promise<{
        solution: any[];
        iterations: number;
        residual: number;
        converged: boolean;
        method: string;
        computeTime: number;
        memoryUsed: number;
    } | {
        solution: number[];
        iterations: number;
        residual: number;
        converged: boolean;
        method: string;
        computeTime: number;
        memoryUsed: number;
        efficiency: {
            convergenceRate: number;
            timePerIteration: number;
            memoryEfficiency: number;
            speedupVsPython: number;
            speedupVsBroken: number;
        };
        metadata: {
            matrixSize: {
                rows: any;
                cols: any;
            };
            sparsity: number;
            nnz: any;
            format: string;
            timestamp: string;
        };
    }>;
    /**
     * Fallback to original solver for unsupported formats
     */
    private static fallbackSolve;
    /**
     * Estimate single entry (simplified)
     */
    static estimateEntry(params: any): Promise<{
        estimate: any;
        variance: number;
        confidence: number;
        standardError: number;
        confidenceInterval: {
            lower: number;
            upper: number;
        };
        row: any;
        column: any;
        method: string;
        metadata: {
            timestamp: string;
        };
    }>;
    /**
     * Batch solve multiple systems
     */
    static batchSolve(matrix: any, vectors: number[][], params?: any): Promise<{
        results: any[];
        summary: {
            totalSystems: number;
            averageTime: number;
            totalTime: number;
        };
    }>;
}
export default OptimizedSolverTools;
