/**
 * MCP Tools for core solver functionality
 */
import { SolveParams, EstimateEntryParams } from '../../core/types.js';
export declare class SolverTools {
    private static wasmSolver;
    /**
     * Get or create WASM solver instance
     */
    private static getWasmSolver;
    /**
     * Determine if we should use the optimized solver
     * Uses optimized solver for dense matrices or when performance is critical
     */
    private static shouldUseOptimizedSolver;
    /**
     * Solve linear system tool
     *
     * PERFORMANCE FIX: Use optimized solver for dense matrices
     * This fixes the 190x slowdown issue (7700ms -> 2.45ms for 1000x1000)
     */
    static solve(params: SolveParams): Promise<any>;
    /**
     * Estimate single entry tool
     */
    static estimateEntry(params: EstimateEntryParams): Promise<{
        estimate: number;
        variance: number;
        confidence: number;
        standardError: number;
        confidenceInterval: {
            lower: number;
            upper: number;
        };
        row: number;
        column: number;
        method: "neumann" | "random-walk" | "monte-carlo";
        metadata: {
            matrixSize: {
                rows: number;
                cols: number;
            };
            configUsed: {
                row: number;
                column: number;
                epsilon: number;
                confidence: number;
                method: "neumann" | "random-walk" | "monte-carlo";
            };
            timestamp: string;
        };
    }>;
    /**
     * Streaming solve for large problems
     */
    static streamingSolve(params: SolveParams, progressCallback?: (progress: any) => void): AsyncGenerator<{
        type: string;
        result: import("../../core/types.js").SolverResult;
        totalIterations: number;
        totalTime: number;
        error?: undefined;
        iterations?: undefined;
        elapsedTime?: undefined;
    } | {
        type: string;
        error: string;
        iterations: number;
        elapsedTime: number;
        result?: undefined;
        totalIterations?: undefined;
        totalTime?: undefined;
    }, void, unknown>;
    /**
     * Batch solve multiple systems with same matrix
     */
    static batchSolve(matrix: any, vectors: number[][], params?: Partial<SolveParams>): Promise<{
        results: any[];
        summary: {
            totalSystems: number;
            averageIterations: number;
            averageTime: number;
            allConverged: boolean;
            convergenceRate: number;
        };
    }>;
}
