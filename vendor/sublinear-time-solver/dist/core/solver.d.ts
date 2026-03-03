/**
 * Core solver algorithms for asymmetric diagonally dominant systems
 * Implements Neumann series, random walks, and push methods
 */
import { Matrix, Vector, SolverConfig, SolverResult, EstimationConfig, PageRankConfig, ProgressCallback } from './types.js';
export declare class SublinearSolver {
    private config;
    private performanceMonitor;
    private convergenceChecker;
    private timeoutController?;
    private wasmAccelerated;
    private wasmModules;
    constructor(config: SolverConfig);
    private initializeWasm;
    private validateConfig;
    /**
     * Solve ADD system Mx = b using specified method
     */
    solve(matrix: Matrix, vector: Vector, progressCallback?: ProgressCallback): Promise<SolverResult>;
    /**
     * Solve using Neumann series expansion
     * x* = (I - D^(-1)R)^(-1) D^(-1) b = sum_{k=0}^∞ (D^(-1)R)^k D^(-1) b
     */
    private solveNeumann;
    /**
     * Compute off-diagonal matrix-vector multiplication: (M - D) * v
     * This computes R*v where R = M - D (off-diagonal part of matrix)
     */
    private computeOffDiagonalMultiply;
    /**
     * Solve using random walk sampling
     */
    private solveRandomWalk;
    /**
     * Create transition matrix for random walks
     */
    private createTransitionMatrix;
    /**
     * Perform a single random walk
     */
    private performRandomWalk;
    /**
     * Solve using forward push method
     */
    private solveForwardPush;
    /**
     * Solve using backward push method
     */
    private solveBackwardPush;
    /**
     * Solve using bidirectional approach (combine forward and backward)
     */
    private solveBidirectional;
    /**
     * Estimate a single entry of the solution M^(-1)b
     */
    estimateEntry(matrix: Matrix, vector: Vector, config: EstimationConfig): Promise<{
        estimate: number;
        variance: number;
        confidence: number;
    }>;
    /**
     * Compute PageRank using the solver
     */
    computePageRank(adjacency: Matrix, config: PageRankConfig): Promise<Vector>;
}
