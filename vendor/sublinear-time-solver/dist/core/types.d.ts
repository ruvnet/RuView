/**
 * Core type definitions for the sublinear-time solver
 */
export interface SparseMatrix {
    rows: number;
    cols: number;
    values: number[];
    rowIndices: number[];
    colIndices: number[];
    format: 'coo' | 'csr' | 'csc';
}
export interface DenseMatrix {
    rows: number;
    cols: number;
    data: number[][];
    format: 'dense';
}
export type Matrix = SparseMatrix | DenseMatrix;
export type Vector = number[];
export interface SolverConfig {
    method: 'neumann' | 'random-walk' | 'forward-push' | 'backward-push' | 'bidirectional';
    epsilon: number;
    maxIterations: number;
    timeout?: number | undefined;
    enableProgress?: boolean | undefined;
    seed?: number | undefined;
}
export interface SolverResult {
    solution: Vector;
    iterations: number;
    residual: number;
    converged: boolean;
    method: string;
    computeTime: number;
    memoryUsed: number;
}
export interface MatrixAnalysis {
    isDiagonallyDominant: boolean;
    dominanceType: 'row' | 'column' | 'none';
    dominanceStrength: number;
    spectralRadius?: number;
    condition?: number;
    pNormGap?: number;
    isSymmetric: boolean;
    sparsity: number;
    size: {
        rows: number;
        cols: number;
    };
}
export interface RandomWalkConfig {
    startNode?: number;
    endNode?: number;
    walkLength: number;
    numWalks: number;
    seed?: number;
}
export interface PageRankConfig {
    damping: number;
    personalized?: Vector;
    epsilon: number;
    maxIterations: number;
}
export interface EstimationConfig {
    row: number;
    column: number;
    epsilon: number;
    confidence: number;
    method: 'neumann' | 'random-walk' | 'monte-carlo';
}
export declare class SolverError extends Error {
    code: string;
    details?: unknown;
    constructor(message: string, code: string, details?: unknown);
}
export declare const ErrorCodes: {
    readonly NOT_DIAGONALLY_DOMINANT: "E001";
    readonly CONVERGENCE_FAILED: "E002";
    readonly INVALID_MATRIX: "E003";
    readonly TIMEOUT: "E004";
    readonly INVALID_DIMENSIONS: "E005";
    readonly NUMERICAL_INSTABILITY: "E006";
    readonly MEMORY_LIMIT_EXCEEDED: "E007";
    readonly INVALID_PARAMETERS: "E008";
};
export type ProgressCallback = (progress: {
    iteration: number;
    residual: number;
    elapsed: number;
    estimated?: number;
}) => void;
export interface SolveParams {
    matrix: Matrix;
    vector: Vector;
    method?: 'neumann' | 'random-walk' | 'forward-push' | 'backward-push' | 'bidirectional' | undefined;
    epsilon?: number | undefined;
    maxIterations?: number | undefined;
    timeout?: number | undefined;
}
export interface EstimateEntryParams {
    matrix: Matrix;
    vector: Vector;
    row: number;
    column: number;
    epsilon: number;
    confidence?: number | undefined;
    method?: 'neumann' | 'random-walk' | 'monte-carlo' | undefined;
}
export interface AnalyzeMatrixParams {
    matrix: Matrix;
    checkDominance?: boolean;
    computeGap?: boolean;
    estimateCondition?: boolean;
    checkSymmetry?: boolean;
}
export interface PageRankParams {
    adjacency: Matrix;
    damping?: number | undefined;
    personalized?: Vector | undefined;
    epsilon?: number | undefined;
    maxIterations?: number | undefined;
}
export interface EffectiveResistanceParams {
    laplacian: Matrix;
    source: number;
    target: number;
    epsilon?: number;
}
export interface AlgorithmState {
    iteration: number;
    residual: number;
    solution: Vector;
    converged: boolean;
    elapsedTime: number;
}
export interface NeumannState extends AlgorithmState {
    series: Vector[];
    convergenceRate: number;
}
export interface RandomWalkState extends AlgorithmState {
    walks: number[][];
    currentEstimate: number;
    variance: number;
    confidence: number;
}
export interface PushState extends AlgorithmState {
    residualVector: Vector;
    approximateVector: Vector;
    pushDirection: 'forward' | 'backward';
}
