// TypeScript definitions for sublinear-time-solver

export interface SolverConfig {
  maxIterations: number;
  tolerance: number;
  simdEnabled: boolean;
  streamChunkSize: number;
}

export interface MatrixData {
  data: Float64Array;
  rows: number;
  cols: number;
}

export interface SolutionStep {
  iteration: number;
  residual: number;
  timestamp: number;
  convergence: boolean;
}

export interface MemoryUsage {
  used: number;
  capacity: number;
  js?: {
    allocations: number;
    totalBytes: number;
    wasmMemory: number;
  };
}

export interface BatchSolveRequest {
  id: string;
  matrix: Matrix;
  vector: Float64Array;
  priority?: number;
}

export interface BatchSolveResult {
  id: string;
  solution: Float64Array;
  iterations: number;
  error?: string;
}

export interface Features {
  simd_enabled: boolean;
  parallel_enabled: boolean;
  memory_64: boolean;
  wee_alloc: boolean;
}

export declare class Matrix {
  data: Float64Array;
  rows: number;
  cols: number;

  constructor(data: Float64Array | number[], rows: number, cols: number);

  static zeros(rows: number, cols: number): Matrix;
  static identity(size: number): Matrix;
  static random(rows: number, cols: number): Matrix;

  get(row: number, col: number): number;
  set(row: number, col: number, value: number): void;
  toWasmView(): MatrixView;
}

export declare class MatrixView {
  constructor(rows: number, cols: number);

  readonly data: number;
  readonly length: number;
  readonly rows: number;
  readonly cols: number;

  data_view(): Float64Array;
  set_data(data: Float64Array): void;
  get_element(row: number, col: number): number;
  set_element(row: number, col: number, value: number): void;
}

export declare class SolutionStream implements AsyncIterableIterator<SolutionStep> {
  constructor(solver: SublinearSolver, matrix: Matrix, vector: Float64Array);

  [Symbol.asyncIterator](): AsyncIterableIterator<SolutionStep>;
  next(): Promise<IteratorResult<SolutionStep>>;
}

export declare class MemoryManager {
  constructor();

  allocateFloat64Array(length: number): { id: string; buffer: Float64Array };
  deallocate(id: string): void;
  getUsage(): {
    allocations: number;
    totalBytes: number;
    wasmMemory: number;
  };
  clear(): void;
}

export declare class SublinearSolver {
  constructor(config?: Partial<SolverConfig>);

  initialize(): Promise<void>;

  // Synchronous solve
  solve(matrix: Matrix, vector: Float64Array): Promise<Float64Array>;

  // Streaming solve
  solveStream(matrix: Matrix, vector: Float64Array): AsyncIterableIterator<SolutionStep>;

  // Batch operations
  solveBatch(problems: Array<{matrix: Matrix, vector: Float64Array}>): Promise<BatchSolveResult[]>;

  // Memory management
  getMemoryUsage(): MemoryUsage;
  getConfig(): SolverConfig;
  dispose(): void;
}

export declare class SolverError extends Error {
  type: string;
  constructor(message: string, type?: string);
}

export declare class MemoryError extends Error {
  type: string;
  constructor(message: string);
}

export declare class ValidationError extends Error {
  type: string;
  constructor(message: string);
}

// Factory function
export declare function createSolver(config?: Partial<SolverConfig>): Promise<SublinearSolver>;

// Utility functions
export declare const Utils: {
  getFeatures(): Promise<Features>;
  isSIMDEnabled(): Promise<boolean>;
  benchmarkMatrixMultiply(size: number): Promise<number>;
  getWasmMemoryUsage(): Promise<number>;
};

// WASM module interface
export interface WasmModule {
  memory: WebAssembly.Memory;
  WasmSublinearSolver: typeof WasmSublinearSolver;
  MatrixView: typeof MatrixView;
  get_features(): Features;
  enable_simd(): boolean;
  get_wasm_memory_usage(): number;
  benchmark_matrix_multiply(size: number): number;
}

// WASM Solver interface
declare class WasmSublinearSolver {
  constructor(config: SolverConfig);

  solve(
    matrix_data: Float64Array,
    matrix_rows: number,
    matrix_cols: number,
    vector_data: Float64Array
  ): Float64Array;

  solve_stream(
    matrix_data: Float64Array,
    matrix_rows: number,
    matrix_cols: number,
    vector_data: Float64Array,
    callback: (step: SolutionStep) => void
  ): Float64Array;

  solve_batch(batch_data: BatchSolveRequest[]): BatchSolveResult[];

  readonly memory_usage: MemoryUsage;
  get_config(): SolverConfig;
  dispose(): void;
}

// Default export
declare const SublinearSolver: typeof SublinearSolver;
export default SublinearSolver;

// Re-exports for convenience
export {
  SublinearSolver,
  Matrix,
  SolutionStep,
  SolutionStream,
  MemoryManager
};

// Module augmentation for environments
declare global {
  interface Window {
    SublinearSolver?: typeof SublinearSolver;
  }
}

// CommonJS compatibility
declare namespace SublinearTimeSolver {
  export {
    SublinearSolver,
    Matrix,
    SolverConfig,
    SolutionStep,
    SolutionStream,
    MemoryManager,
    BatchSolveRequest,
    BatchSolveResult,
    MemoryUsage,
    Features,
    SolverError,
    MemoryError,
    ValidationError,
    createSolver,
    Utils
  };
}

export = SublinearTimeSolver;