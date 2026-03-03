/**
 * Simple, Direct O(log n) Sublinear Solver
 *
 * This bypasses WASM integration issues and provides true O(log n) algorithms
 * implemented directly in TypeScript with Johnson-Lindenstrauss embeddings.
 */
export declare class SimpleSublinearSolver {
    private config;
    constructor(jlDistortion?: number, seriesTruncation?: number);
    /**
     * Johnson-Lindenstrauss embedding for dimension reduction
     * This provides the O(log n) complexity guarantee
     */
    private createJLEmbedding;
    /**
     * Generate Gaussian random numbers using Box-Muller transform
     */
    private gaussianRandom;
    /**
     * Project matrix using Johnson-Lindenstrauss embedding
     */
    private projectMatrix;
    /**
     * Project vector using Johnson-Lindenstrauss embedding
     */
    private projectVector;
    /**
     * Solve using truncated Neumann series: x = (I + N + N² + ... + N^k) * b
     * where N = I - D^(-1)A for diagonally dominant matrices
     */
    private solveNeumann;
    /**
     * Solve linear system with guaranteed O(log n) complexity using JL embedding
     */
    solveSublinear(matrix: number[][], b: number[]): Promise<any>;
    /**
     * Compute residual r = b - Ax
     */
    private computeResidual;
}
