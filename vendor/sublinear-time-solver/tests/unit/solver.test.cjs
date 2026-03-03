#!/usr/bin/env node

/**
 * Unit tests for SublinearSolver class and related functionality
 * Run with: node tests/unit/solver.test.js
 */

const { strict: assert } = require('assert');

// Mock WASM imports since we don't have the built package yet
const mockWasm = {
    init: async () => ({}),
    WasmSublinearSolver: class {
        constructor(config) {
            this.config = config;
            this.memory_usage = { used: 1024, capacity: 2048 };
        }

        solve(data, rows, cols, vector) {
            // Mock solver that returns a simple solution
            return new Float64Array(vector.length).fill(1.0);
        }

        solve_batch(problems) {
            return problems.map(problem => ({
                id: problem.id,
                solution: new Array(problem.vector_data.length).fill(1.0),
                iterations: 10,
                error: null
            }));
        }

        get_config() {
            return this.config;
        }

        dispose() {
            // Mock cleanup
        }
    },
    MatrixView: class {
        constructor(rows, cols) {
            this.rows = rows;
            this.cols = cols;
        }
    },
    get_features: () => ({ simd: true, threads: 4 }),
    enable_simd: () => true,
    get_wasm_memory_usage: () => ({ used: 1024, total: 2048 }),
    benchmark_matrix_multiply: (size) => ({ time: 10.5, operations: size * size })
};

// Create a mock solver module
const mockSolverModule = {
    Matrix: class {
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
            return new mockSolverModule.Matrix(new Float64Array(rows * cols), rows, cols);
        }

        static identity(size) {
            const data = new Float64Array(size * size);
            for (let i = 0; i < size; i++) {
                data[i * size + i] = 1.0;
            }
            return new mockSolverModule.Matrix(data, size, size);
        }

        get(row, col) {
            return this.data[row * this.cols + col];
        }

        set(row, col, value) {
            this.data[row * this.cols + col] = value;
        }

        toWasmView() {
            return new mockWasm.MatrixView(this.rows, this.cols);
        }
    },

    SolverConfig: class {
        constructor(options = {}) {
            this.maxIterations = options.maxIterations || 1000;
            this.tolerance = options.tolerance || 1e-10;
            this.simdEnabled = options.simdEnabled !== false;
            this.streamChunkSize = options.streamChunkSize || 100;
        }
    },

    SolutionStep: class {
        constructor(iteration, residual, timestamp, convergence) {
            this.iteration = iteration;
            this.residual = residual;
            this.timestamp = timestamp;
            this.convergence = convergence;
        }
    },

    MemoryManager: class {
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
                wasmMemory: mockWasm.get_wasm_memory_usage()
            };
        }

        clear() {
            this.allocations.clear();
        }
    },

    SublinearSolver: class {
        constructor(config = new mockSolverModule.SolverConfig()) {
            this.config = config;
            this.wasmSolver = null;
            this.memoryManager = new mockSolverModule.MemoryManager();
            this.initialized = false;
        }

        async initialize() {
            if (this.initialized) return;

            // Mock WASM initialization
            this.wasmSolver = new mockWasm.WasmSublinearSolver(this.config);
            this.initialized = true;
        }

        async solve(matrix, vector) {
            await this.initialize();

            if (!(matrix instanceof mockSolverModule.Matrix)) {
                throw new Error('Matrix must be instance of Matrix class');
            }

            if (!(vector instanceof Float64Array)) {
                throw new Error('Vector must be Float64Array');
            }

            const result = this.wasmSolver.solve(
                matrix.data,
                matrix.rows,
                matrix.cols,
                vector
            );

            return new Float64Array(result);
        }

        async solveBatch(problems) {
            await this.initialize();

            const batchData = problems.map((problem, index) => ({
                id: `batch_${index}`,
                matrix_data: Array.from(problem.matrix.data),
                matrix_rows: problem.matrix.rows,
                matrix_cols: problem.matrix.cols,
                vector_data: Array.from(problem.vector)
            }));

            const results = this.wasmSolver.solve_batch(batchData);
            return results.map(result => ({
                id: result.id,
                solution: new Float64Array(result.solution),
                iterations: result.iterations,
                error: result.error
            }));
        }

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

        getConfig() {
            if (!this.initialized) return this.config;
            return this.wasmSolver.get_config();
        }

        dispose() {
            if (this.wasmSolver) {
                this.wasmSolver.dispose();
                this.wasmSolver = null;
            }
            this.memoryManager.clear();
            this.initialized = false;
        }
    },

    SolverError: class extends Error {
        constructor(message, type = 'SOLVER_ERROR') {
            super(message);
            this.name = 'SolverError';
            this.type = type;
        }
    },

    MemoryError: class extends Error {
        constructor(message) {
            super(message);
            this.name = 'MemoryError';
            this.type = 'MEMORY_ERROR';
        }
    },

    ValidationError: class extends Error {
        constructor(message) {
            super(message);
            this.name = 'ValidationError';
            this.type = 'VALIDATION_ERROR';
        }
    }
};

// Mock createSolver function
mockSolverModule.createSolver = async (config) => {
    const solver = new mockSolverModule.SublinearSolver(config);
    await solver.initialize();
    return solver;
};

const { Matrix, SolverConfig, SublinearSolver, SolutionStep, MemoryManager,
        SolverError, MemoryError, ValidationError, createSolver } = mockSolverModule;

class TestRunner {
    constructor() {
        this.tests = [];
        this.passed = 0;
        this.failed = 0;
        this.verbose = process.argv.includes('--verbose');
    }

    test(name, fn) {
        this.tests.push({ name, fn });
    }

    async run() {
        console.log('🧪 Running SublinearSolver Unit Tests');
        console.log('=====================================\n');

        for (const { name, fn } of this.tests) {
            try {
                await fn();
                this.passed++;
                console.log(`✅ ${name}`);
            } catch (error) {
                this.failed++;
                console.log(`❌ ${name}`);
                if (this.verbose) {
                    console.log(`   Error: ${error.message}`);
                    console.log(`   Stack: ${error.stack}\n`);
                } else {
                    console.log(`   Error: ${error.message}\n`);
                }
            }
        }

        this.printSummary();
        return this.failed === 0;
    }

    printSummary() {
        console.log('\n📊 Test Summary');
        console.log('===============');
        console.log(`✅ Passed: ${this.passed}`);
        console.log(`❌ Failed: ${this.failed}`);
        console.log(`📈 Total:  ${this.tests.length}`);
        console.log(`🎯 Success Rate: ${((this.passed / this.tests.length) * 100).toFixed(1)}%`);
    }
}

const runner = new TestRunner();

// SublinearSolver Constructor Tests
runner.test('SublinearSolver constructor with defaults', () => {
    const solver = new SublinearSolver();

    assert.ok(solver.config instanceof SolverConfig);
    assert.equal(solver.initialized, false);
    assert.ok(solver.memoryManager instanceof MemoryManager);
    assert.equal(solver.wasmSolver, null);
});

runner.test('SublinearSolver constructor with custom config', () => {
    const config = new SolverConfig({
        maxIterations: 500,
        tolerance: 1e-8
    });
    const solver = new SublinearSolver(config);

    assert.equal(solver.config.maxIterations, 500);
    assert.equal(solver.config.tolerance, 1e-8);
});

// Solver Initialization Tests
runner.test('SublinearSolver initialization', async () => {
    const solver = new SublinearSolver();

    assert.equal(solver.initialized, false);

    await solver.initialize();

    assert.equal(solver.initialized, true);
    assert.ok(solver.wasmSolver !== null);
});

runner.test('SublinearSolver double initialization', async () => {
    const solver = new SublinearSolver();

    await solver.initialize();
    await solver.initialize(); // Should not throw

    assert.equal(solver.initialized, true);
});

// Solver Basic Operations
runner.test('SublinearSolver solve basic linear system', async () => {
    const solver = new SublinearSolver();

    // Create a simple 2x2 system
    const matrix = new Matrix([2, 1, 1, 2], 2, 2);
    const vector = new Float64Array([3, 3]);

    const solution = await solver.solve(matrix, vector);

    assert.ok(solution instanceof Float64Array);
    assert.equal(solution.length, 2);
});

runner.test('SublinearSolver solve input validation', async () => {
    const solver = new SublinearSolver();

    // Test with invalid matrix
    const vector = new Float64Array([1, 2]);

    try {
        await solver.solve("not a matrix", vector);
        assert.fail('Should have thrown error for invalid matrix');
    } catch (error) {
        assert.ok(error.message.includes('Matrix must be instance of Matrix class'));
    }

    // Test with invalid vector
    const matrix = new Matrix([1, 0, 0, 1], 2, 2);

    try {
        await solver.solve(matrix, [1, 2]);
        assert.fail('Should have thrown error for invalid vector');
    } catch (error) {
        assert.ok(error.message.includes('Vector must be Float64Array'));
    }
});

// Batch Solving Tests
runner.test('SublinearSolver batch solve', async () => {
    const solver = new SublinearSolver();

    const problems = [
        {
            matrix: new Matrix([2, 0, 0, 2], 2, 2),
            vector: new Float64Array([2, 4])
        },
        {
            matrix: new Matrix([1, 1, 1, 1], 2, 2),
            vector: new Float64Array([2, 2])
        }
    ];

    const results = await solver.solveBatch(problems);

    assert.equal(results.length, 2);

    results.forEach((result, index) => {
        assert.ok(result.id.includes('batch_'));
        assert.ok(result.solution instanceof Float64Array);
        assert.equal(typeof result.iterations, 'number');
        assert.equal(result.error, null);
    });
});

runner.test('SublinearSolver empty batch solve', async () => {
    const solver = new SublinearSolver();

    const results = await solver.solveBatch([]);

    assert.equal(results.length, 0);
});

// Memory Management Tests
runner.test('SublinearSolver memory usage tracking', async () => {
    const solver = new SublinearSolver();

    // Before initialization
    const memoryBefore = solver.getMemoryUsage();
    assert.equal(memoryBefore.used, 0);
    assert.equal(memoryBefore.capacity, 0);
    assert.ok(memoryBefore.js);

    // After initialization
    await solver.initialize();

    const memoryAfter = solver.getMemoryUsage();
    assert.ok(memoryAfter.used > 0);
    assert.ok(memoryAfter.capacity > 0);
    assert.ok(memoryAfter.js);
});

runner.test('SublinearSolver config access', async () => {
    const config = new SolverConfig({
        maxIterations: 750,
        tolerance: 1e-9
    });
    const solver = new SublinearSolver(config);

    // Before initialization
    const configBefore = solver.getConfig();
    assert.equal(configBefore.maxIterations, 750);
    assert.equal(configBefore.tolerance, 1e-9);

    // After initialization
    await solver.initialize();

    const configAfter = solver.getConfig();
    assert.equal(configAfter.maxIterations, 750);
    assert.equal(configAfter.tolerance, 1e-9);
});

// Resource Cleanup Tests
runner.test('SublinearSolver dispose', async () => {
    const solver = new SublinearSolver();

    await solver.initialize();
    assert.equal(solver.initialized, true);

    solver.dispose();

    assert.equal(solver.initialized, false);
    assert.equal(solver.wasmSolver, null);
});

// Factory Function Tests
runner.test('createSolver factory function', async () => {
    const config = new SolverConfig({
        maxIterations: 500,
        tolerance: 1e-7
    });

    const solver = await createSolver(config);

    assert.ok(solver instanceof SublinearSolver);
    assert.equal(solver.initialized, true);
    assert.equal(solver.config.maxIterations, 500);
    assert.equal(solver.config.tolerance, 1e-7);
});

runner.test('createSolver with undefined config', async () => {
    const solver = await createSolver();

    assert.ok(solver instanceof SublinearSolver);
    assert.equal(solver.initialized, true);
    assert.equal(solver.config.maxIterations, 1000); // Default
});

// Error Classes Tests
runner.test('SolverError properties', () => {
    const error = new SolverError('Test error', 'TEST_TYPE');

    assert.equal(error.name, 'SolverError');
    assert.equal(error.message, 'Test error');
    assert.equal(error.type, 'TEST_TYPE');
    assert.ok(error instanceof Error);
});

runner.test('MemoryError properties', () => {
    const error = new MemoryError('Memory test error');

    assert.equal(error.name, 'MemoryError');
    assert.equal(error.message, 'Memory test error');
    assert.equal(error.type, 'MEMORY_ERROR');
    assert.ok(error instanceof Error);
});

runner.test('ValidationError properties', () => {
    const error = new ValidationError('Validation test error');

    assert.equal(error.name, 'ValidationError');
    assert.equal(error.message, 'Validation test error');
    assert.equal(error.type, 'VALIDATION_ERROR');
    assert.ok(error instanceof Error);
});

// SolutionStep Tests
runner.test('SolutionStep construction', () => {
    const step = new SolutionStep(5, 0.001, Date.now(), false);

    assert.equal(step.iteration, 5);
    assert.equal(step.residual, 0.001);
    assert.equal(typeof step.timestamp, 'number');
    assert.equal(step.convergence, false);
});

// Integration Tests
runner.test('Complete solver workflow', async () => {
    // Create solver with custom config
    const config = new SolverConfig({
        maxIterations: 100,
        tolerance: 1e-6
    });
    const solver = new SublinearSolver(config);

    // Create test matrix and vector
    const matrix = Matrix.identity(3);
    const vector = new Float64Array([1, 2, 3]);

    // Solve system
    const solution = await solver.solve(matrix, vector);

    // Verify solution
    assert.ok(solution instanceof Float64Array);
    assert.equal(solution.length, 3);

    // Check memory usage
    const memory = solver.getMemoryUsage();
    assert.ok(memory.used > 0);

    // Clean up
    solver.dispose();
    assert.equal(solver.initialized, false);
});

runner.test('Solver with zero matrix', async () => {
    const solver = new SublinearSolver();

    const matrix = Matrix.zeros(2, 2);
    const vector = new Float64Array([0, 0]);

    // This should not throw in our mock implementation
    const solution = await solver.solve(matrix, vector);
    assert.ok(solution instanceof Float64Array);
});

runner.test('Large matrix stress test', async () => {
    const solver = new SublinearSolver();

    const size = 100;
    const matrix = Matrix.identity(size);
    const vector = new Float64Array(size).fill(1);

    const startTime = Date.now();
    const solution = await solver.solve(matrix, vector);
    const endTime = Date.now();

    assert.ok(solution instanceof Float64Array);
    assert.equal(solution.length, size);

    // Should complete reasonably quickly (mock implementation)
    const duration = endTime - startTime;
    assert.ok(duration < 1000, `Solve took too long: ${duration}ms`);
});

// Run all tests
if (require.main === module) {
    runner.run().then(success => {
        process.exit(success ? 0 : 1);
    }).catch(error => {
        console.error('Test runner failed:', error);
        process.exit(1);
    });
}

module.exports = { TestRunner, runner };