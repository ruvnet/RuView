/**
 * Core type definitions for the sublinear-time solver
 */

// Matrix representations
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

// Vector type
export type Vector = number[];

// Solver configuration
export interface SolverConfig {
  method: 'neumann' | 'random-walk' | 'forward-push' | 'backward-push' | 'bidirectional';
  epsilon: number;
  maxIterations: number;
  timeout?: number | undefined;
  enableProgress?: boolean | undefined;
  seed?: number | undefined;
}

// Solver result
export interface SolverResult {
  solution: Vector;
  iterations: number;
  residual: number;
  converged: boolean;
  method: string;
  computeTime: number;
  memoryUsed: number;
}

// Matrix analysis result
export interface MatrixAnalysis {
  isDiagonallyDominant: boolean;
  dominanceType: 'row' | 'column' | 'none';
  dominanceStrength: number;
  spectralRadius?: number;
  condition?: number;
  pNormGap?: number;
  isSymmetric: boolean;
  sparsity: number;
  size: { rows: number; cols: number };
}

// Random walk configuration
export interface RandomWalkConfig {
  startNode?: number;
  endNode?: number;
  walkLength: number;
  numWalks: number;
  seed?: number;
}

// PageRank configuration
export interface PageRankConfig {
  damping: number;
  personalized?: Vector;
  epsilon: number;
  maxIterations: number;
}

// Estimation configuration
export interface EstimationConfig {
  row: number;
  column: number;
  epsilon: number;
  confidence: number;
  method: 'neumann' | 'random-walk' | 'monte-carlo';
}

// Error types
export class SolverError extends Error {
  constructor(
    message: string,
    public code: string,
    public details?: unknown
  ) {
    super(message);
    this.name = 'SolverError';
  }
}

export const ErrorCodes = {
  NOT_DIAGONALLY_DOMINANT: 'E001',
  CONVERGENCE_FAILED: 'E002',
  INVALID_MATRIX: 'E003',
  TIMEOUT: 'E004',
  INVALID_DIMENSIONS: 'E005',
  NUMERICAL_INSTABILITY: 'E006',
  MEMORY_LIMIT_EXCEEDED: 'E007',
  INVALID_PARAMETERS: 'E008'
} as const;

// Progress callback type
export type ProgressCallback = (progress: {
  iteration: number;
  residual: number;
  elapsed: number;
  estimated?: number;
}) => void;

// MCP Tool parameter types
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

// Internal algorithm state
export interface AlgorithmState {
  iteration: number;
  residual: number;
  solution: Vector;
  converged: boolean;
  elapsedTime: number;
}

// Neumann series state
export interface NeumannState extends AlgorithmState {
  series: Vector[];
  convergenceRate: number;
}

// Random walk state
export interface RandomWalkState extends AlgorithmState {
  walks: number[][];
  currentEstimate: number;
  variance: number;
  confidence: number;
}

// Push algorithm state
export interface PushState extends AlgorithmState {
  residualVector: Vector;
  approximateVector: Vector;
  pushDirection: 'forward' | 'backward';
}