import init, {
    WasmSublinearSolver,
    MatrixView,
    get_features,
    enable_simd,
    get_wasm_memory_usage,
    benchmark_matrix_multiply
} from '../pkg/sublinear_time_solver.js';

// Initialize WebAssembly module
let wasmInitialized = false;
let wasmModule = null;

async function ensureWasmInitialized() {
    if (!wasmInitialized) {
        wasmModule = await init();
        wasmInitialized = true;
    }
    return wasmModule;
}

/**
 * Configuration interface for the solver
 */
export class SolverConfig {
    constructor(options = {}) {
        this.maxIterations = options.maxIterations || 1000;
        this.tolerance = options.tolerance || 1e-10;
        this.simdEnabled = options.simdEnabled !== false;
        this.streamChunkSize = options.streamChunkSize || 100;
    }
}

/**
 * Matrix class for efficient data handling
 */
export class Matrix {
    constructor(data, rows, cols) {
        if (data instanceof Float64Array) {
            this.data = data;
        } else if (Array.isArray(data)) {
            this.data = new Float64Array(data);
        } else {
            throw new Error('Matrix data must be Float64Array or Array');
        }

        this.rows = rows;
        this.cols = cols;

        if (this.data.length !== rows * cols) {
            throw new Error('Data length must match matrix dimensions');
        }
    }

    static zeros(rows, cols) {
        return new Matrix(new Float64Array(rows * cols), rows, cols);
    }

    static identity(size) {
        const data = new Float64Array(size * size);
        for (let i = 0; i < size; i++) {
            data[i * size + i] = 1.0;
        }
        return new Matrix(data, size, size);
    }

    static random(rows, cols) {
        const data = new Float64Array(rows * cols);
        for (let i = 0; i < data.length; i++) {
            data[i] = Math.random();
        }
        return new Matrix(data, rows, cols);
    }

    get(row, col) {
        return this.data[row * this.cols + col];
    }

    set(row, col, value) {
        this.data[row * this.cols + col] = value;
    }

    toWasmView() {
        return new MatrixView(this.rows, this.cols);
    }
}

/**
 * Solution step information for streaming interface
 */
export class SolutionStep {
    constructor(iteration, residual, timestamp, convergence) {
        this.iteration = iteration;
        this.residual = residual;
        this.timestamp = timestamp;
        this.convergence = convergence;
    }
}

/**
 * Streaming solver using AsyncIterator pattern
 */
export class SolutionStream {
    constructor(solver, matrix, vector) {
        this.solver = solver;
        this.matrix = matrix;
        this.vector = vector;
        this.buffer = [];
        this.isComplete = false;
        this.error = null;
    }

    async *[Symbol.asyncIterator]() {
        try {
            const solution = await new Promise((resolve, reject) => {
                this.solver.wasmSolver.solve_stream(
                    this.matrix.data,
                    this.matrix.rows,
                    this.matrix.cols,
                    this.vector,
                    (stepData) => {
                        const step = new SolutionStep(
                            stepData.iteration,
                            stepData.residual,
                            stepData.timestamp,
                            stepData.convergence
                        );
                        this.buffer.push(step);
                    }
                );

                // Process buffered steps
                this.processBuffer(resolve, reject);
            });

            // Yield all buffered steps
            while (this.buffer.length > 0) {
                yield this.buffer.shift();
            }

        } catch (error) {
            throw new Error(`Streaming solve failed: ${error.message}`);
        }
    }

    async processBuffer(resolve, reject) {
        // Simple processing - in production this would be more sophisticated
        const checkBuffer = () => {
            if (this.buffer.length > 0) {
                const lastStep = this.buffer[this.buffer.length - 1];
                if (lastStep.convergence) {
                    resolve();
                    return;
                }
            }
            setTimeout(checkBuffer, 10);
        };
        checkBuffer();
    }
}

/**
 * Memory manager for efficient WASM memory usage
 */
export class MemoryManager {
    constructor() {
        this.allocations = new Map();
    }

    allocateFloat64Array(length) {
        const buffer = new Float64Array(length);
        const id = Math.random().toString(36);
        this.allocations.set(id, buffer);
        return { id, buffer };
    }

    deallocate(id) {
        this.allocations.delete(id);
    }

    getUsage() {
        let totalBytes = 0;
        for (const buffer of this.allocations.values()) {
            totalBytes += buffer.byteLength;
        }
        return {
            allocations: this.allocations.size,
            totalBytes,
            wasmMemory: get_wasm_memory_usage()
        };
    }

    clear() {
        this.allocations.clear();
    }
}

/**
 * Main SublinearSolver class with WASM backend
 */
export class SublinearSolver {
    constructor(config = new SolverConfig()) {
        this.config = config;
        this.wasmSolver = null;
        this.memoryManager = new MemoryManager();
        this.initialized = false;
    }

    async initialize() {
        if (this.initialized) return;

        await ensureWasmInitialized();

        try {
            this.wasmSolver = new WasmSublinearSolver(this.config);
            this.initialized = true;
        } catch (error) {
            throw new Error(`Failed to initialize WASM solver: ${error.message}`);
        }
    }

    /**
     * Solve linear system Ax = b synchronously
     */
    async solve(matrix, vector) {
        await this.initialize();

        if (!(matrix instanceof Matrix)) {
            throw new Error('Matrix must be instance of Matrix class');
        }

        if (!(vector instanceof Float64Array)) {
            throw new Error('Vector must be Float64Array');
        }

        try {
            const result = this.wasmSolver.solve(
                matrix.data,
                matrix.rows,
                matrix.cols,
                vector
            );

            return new Float64Array(result);
        } catch (error) {
            throw new Error(`Solve failed: ${error.message}`);
        }
    }

    /**
     * Solve with streaming progress updates
     */
    async *solveStream(matrix, vector) {
        await this.initialize();

        const stream = new SolutionStream(this, matrix, vector);
        for await (const step of stream) {
            yield step;
        }
    }

    /**
     * Solve batch of problems efficiently
     */
    async solveBatch(problems) {
        await this.initialize();

        const batchData = problems.map((problem, index) => ({
            id: `batch_${index}`,
            matrix_data: Array.from(problem.matrix.data),
            matrix_rows: problem.matrix.rows,
            matrix_cols: problem.matrix.cols,
            vector_data: Array.from(problem.vector)
        }));

        try {
            const results = this.wasmSolver.solve_batch(batchData);
            return results.map(result => ({
                id: result.id,
                solution: new Float64Array(result.solution),
                iterations: result.iterations,
                error: result.error
            }));
        } catch (error) {
            throw new Error(`Batch solve failed: ${error.message}`);
        }
    }

    /**
     * Get current memory usage
     */
    getMemoryUsage() {
        if (!this.initialized) {
            return { used: 0, capacity: 0, js: this.memoryManager.getUsage() };
        }

        const wasmUsage = this.wasmSolver.memory_usage;
        const jsUsage = this.memoryManager.getUsage();

        return {
            used: wasmUsage.used,
            capacity: wasmUsage.capacity,
            js: jsUsage
        };
    }

    /**
     * Get solver configuration
     */
    getConfig() {
        if (!this.initialized) return this.config;
        return this.wasmSolver.get_config();
    }

    /**
     * Clean up resources
     */
    dispose() {
        if (this.wasmSolver) {
            this.wasmSolver.dispose();
            this.wasmSolver = null;
        }
        this.memoryManager.clear();
        this.initialized = false;
    }
}

/**
 * Factory function for easy initialization
 */
export async function createSolver(config) {
    const solver = new SublinearSolver(config);
    await solver.initialize();
    return solver;
}

/**
 * Utility functions
 */
export const Utils = {
    async getFeatures() {
        await ensureWasmInitialized();
        return get_features();
    },

    async isSIMDEnabled() {
        await ensureWasmInitialized();
        return enable_simd();
    },

    async benchmarkMatrixMultiply(size) {
        await ensureWasmInitialized();
        return benchmark_matrix_multiply(size);
    },

    async getWasmMemoryUsage() {
        await ensureWasmInitialized();
        return get_wasm_memory_usage();
    }
};

/**
 * Error classes
 */
export class SolverError extends Error {
    constructor(message, type = 'SOLVER_ERROR') {
        super(message);
        this.name = 'SolverError';
        this.type = type;
    }
}

export class MemoryError extends Error {
    constructor(message) {
        super(message);
        this.name = 'MemoryError';
        this.type = 'MEMORY_ERROR';
    }
}

export class ValidationError extends Error {
    constructor(message) {
        super(message);
        this.name = 'ValidationError';
        this.type = 'VALIDATION_ERROR';
    }
}

// Export everything
export {
    Matrix,
    SolverConfig,
    SolutionStep,
    SolutionStream,
    MemoryManager,
    SublinearSolver as default
};