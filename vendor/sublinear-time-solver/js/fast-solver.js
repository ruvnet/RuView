/**
 * Fast Node.js solver implementation optimized to beat Python benchmarks
 *
 * This addresses the critical MCP Dense performance issue that's 190x slower than Python.
 * Key optimizations:
 * - Native sparse CSR format
 * - Manual loop unrolling
 * - Memory-efficient data structures
 * - Prepared for WASM integration
 */

class FastCSRMatrix {
    constructor(values, colIndices, rowPtr, rows, cols) {
        this.values = new Float64Array(values);
        this.colIndices = new Uint32Array(colIndices);
        this.rowPtr = new Uint32Array(rowPtr);
        this.rows = rows;
        this.cols = cols;
    }

    static fromTriplets(triplets, rows, cols) {
        // Sort triplets by row, then column for optimal CSR construction
        triplets.sort((a, b) => {
            if (a[0] !== b[0]) return a[0] - b[0];
            return a[1] - b[1];
        });

        const values = [];
        const colIndices = [];
        const rowPtr = new Array(rows + 1).fill(0);

        let currentRow = 0;
        for (const [row, col, val] of triplets) {
            // Fill row pointers
            while (currentRow <= row) {
                rowPtr[currentRow] = values.length;
                currentRow++;
            }

            values.push(val);
            colIndices.push(col);
        }

        // Fill remaining row pointers
        while (currentRow <= rows) {
            rowPtr[currentRow] = values.length;
            currentRow++;
        }

        return new FastCSRMatrix(values, colIndices, rowPtr, rows, cols);
    }

    /**
     * Ultra-fast matrix-vector multiplication optimized for performance
     * This is the critical operation that needs to beat Python
     */
    multiplyVector(x, y) {
        // Fill output with zeros
        y.fill(0.0);

        // Process rows with manual loop unrolling
        for (let row = 0; row < this.rows; row++) {
            const start = this.rowPtr[row];
            const end = this.rowPtr[row + 1];
            const nnz = end - start;

            if (nnz === 0) continue;

            // For small rows, use simple accumulation
            if (nnz <= 4) {
                let sum = 0.0;
                for (let idx = start; idx < end; idx++) {
                    sum += this.values[idx] * x[this.colIndices[idx]];
                }
                y[row] = sum;
            } else {
                // For larger rows, unroll loop for better performance
                const chunks = Math.floor(nnz / 4);
                const remainder = nnz % 4;
                let sum = 0.0;

                // Process 4 elements at a time
                let idx = start;
                for (let chunk = 0; chunk < chunks; chunk++) {
                    sum += this.values[idx] * x[this.colIndices[idx]] +
                           this.values[idx + 1] * x[this.colIndices[idx + 1]] +
                           this.values[idx + 2] * x[this.colIndices[idx + 2]] +
                           this.values[idx + 3] * x[this.colIndices[idx + 3]];
                    idx += 4;
                }

                // Handle remainder
                for (let i = 0; i < remainder; i++) {
                    sum += this.values[idx] * x[this.colIndices[idx]];
                    idx++;
                }

                y[row] = sum;
            }
        }
    }

    get nnz() {
        return this.values.length;
    }
}

class FastConjugateGradient {
    constructor(maxIterations = 1000, tolerance = 1e-10) {
        this.maxIterations = maxIterations;
        this.tolerance = tolerance;
        this.toleranceSq = tolerance * tolerance;
    }

    /**
     * Solve Ax = b using optimized conjugate gradient
     * Targets sub-50ms performance for 1000x1000 matrices
     */
    solve(matrix, b) {
        const n = matrix.rows;
        if (matrix.rows !== matrix.cols) {
            throw new Error('Matrix must be square');
        }
        if (b.length !== n) {
            throw new Error('Vector size mismatch');
        }

        // Pre-allocate all vectors with Float64Array for better performance
        const x = new Float64Array(n);
        const r = new Float64Array(b);  // r = b - A*x (initially r = b since x = 0)
        const p = new Float64Array(b);  // p = r initially
        const ap = new Float64Array(n);

        let rsold = this.dotProductFast(r, r);

        for (let iteration = 0; iteration < this.maxIterations; iteration++) {
            if (rsold <= this.toleranceSq) {
                break;
            }

            // ap = A * p
            matrix.multiplyVector(p, ap);

            // alpha = rsold / (p^T * ap)
            const pap = this.dotProductFast(p, ap);
            if (Math.abs(pap) < 1e-16) {
                break;
            }

            const alpha = rsold / pap;

            // x = x + alpha * p
            this.axpyFast(alpha, p, x);

            // r = r - alpha * ap
            this.axpyFast(-alpha, ap, r);

            const rsnew = this.dotProductFast(r, r);
            const beta = rsnew / rsold;

            // p = r + beta * p
            for (let i = 0; i < n; i++) {
                p[i] = r[i] + beta * p[i];
            }

            rsold = rsnew;
        }

        return Array.from(x);
    }

    /**
     * Fast dot product with manual unrolling
     */
    dotProductFast(x, y) {
        const n = x.length;
        const chunks = Math.floor(n / 4);
        const remainder = n % 4;
        let sum = 0.0;

        // Process 4 elements at a time
        let i = 0;
        for (let chunk = 0; chunk < chunks; chunk++) {
            sum += x[i] * y[i] +
                   x[i + 1] * y[i + 1] +
                   x[i + 2] * y[i + 2] +
                   x[i + 3] * y[i + 3];
            i += 4;
        }

        // Handle remainder
        for (let j = 0; j < remainder; j++) {
            sum += x[i] * y[i];
            i++;
        }

        return sum;
    }

    /**
     * Fast AXPY operation: y = alpha * x + y
     */
    axpyFast(alpha, x, y) {
        const n = x.length;
        const chunks = Math.floor(n / 4);
        const remainder = n % 4;

        // Process 4 elements at a time
        let i = 0;
        for (let chunk = 0; chunk < chunks; chunk++) {
            y[i] += alpha * x[i];
            y[i + 1] += alpha * x[i + 1];
            y[i + 2] += alpha * x[i + 2];
            y[i + 3] += alpha * x[i + 3];
            i += 4;
        }

        // Handle remainder
        for (let j = 0; j < remainder; j++) {
            y[i] += alpha * x[i];
            i++;
        }
    }
}

/**
 * Memory-efficient buffer pool for vector reuse
 */
class VectorPool {
    constructor(size, capacity = 8) {
        this.size = size;
        this.buffers = [];

        // Pre-allocate buffers
        for (let i = 0; i < capacity; i++) {
            this.buffers.push(new Float64Array(size));
        }
    }

    getBuffer() {
        return this.buffers.pop() || new Float64Array(this.size);
    }

    returnBuffer(buffer) {
        if (buffer.length === this.size && this.buffers.length < 8) {
            buffer.fill(0.0);
            this.buffers.push(buffer);
        }
    }
}

/**
 * WASM-ready solver interface
 * Prepares for WASM integration when the module becomes available
 */
class FastSolver {
    constructor(config = {}) {
        this.maxIterations = config.maxIterations || 1000;
        this.tolerance = config.tolerance || 1e-10;
        this.useWasm = config.useWasm && this.isWasmAvailable();
        this.vectorPool = null;
    }

    isWasmAvailable() {
        // Check if WASM module is loaded
        try {
            return typeof WebAssembly !== 'undefined' &&
                   global.wasmSolver !== undefined;
        } catch (e) {
            return false;
        }
    }

    /**
     * Create optimized sparse matrix from triplets
     */
    createMatrix(triplets, rows, cols) {
        if (this.useWasm) {
            // Use WASM implementation when available
            return this.createWasmMatrix(triplets, rows, cols);
        } else {
            // Use fast JavaScript implementation
            return FastCSRMatrix.fromTriplets(triplets, rows, cols);
        }
    }

    createWasmMatrix(triplets, rows, cols) {
        // Placeholder for WASM integration
        // This would call the actual WASM module
        console.log('WASM matrix creation not yet implemented, falling back to JS');
        return FastCSRMatrix.fromTriplets(triplets, rows, cols);
    }

    /**
     * Solve linear system with optimal method selection
     */
    solve(matrix, b, options = {}) {
        const startTime = process.hrtime.bigint();

        // Initialize vector pool if needed
        if (!this.vectorPool) {
            this.vectorPool = new VectorPool(matrix.rows);
        }

        let result;
        if (this.useWasm) {
            result = this.solveWasm(matrix, b, options);
        } else {
            const solver = new FastConjugateGradient(this.maxIterations, this.tolerance);
            result = solver.solve(matrix, b);
        }

        const endTime = process.hrtime.bigint();
        const executionTime = Number(endTime - startTime) / 1e6; // Convert to milliseconds

        return {
            solution: result,
            executionTime,
            iterations: this.lastIterations || 0,
            method: this.useWasm ? 'wasm' : 'javascript'
        };
    }

    solveWasm(matrix, b, options) {
        // Placeholder for WASM solver integration
        console.log('WASM solver not yet implemented, falling back to JS');
        const solver = new FastConjugateGradient(this.maxIterations, this.tolerance);
        return solver.solve(matrix, b);
    }

    /**
     * Generate test matrices for benchmarking
     */
    generateTestMatrix(size, sparsity = 0.01) {
        const triplets = [];

        // Generate diagonally dominant sparse matrix
        for (let i = 0; i < size; i++) {
            // Diagonal element (make it dominant)
            const diagVal = 5.0 + i * 0.01;
            triplets.push([i, i, diagVal]);

            // Off-diagonal elements
            const nnzPerRow = Math.max(1, Math.floor(size * sparsity));
            for (let k = 0; k < Math.min(nnzPerRow, 5); k++) {
                const j = Math.floor(Math.random() * size);
                if (i !== j) {
                    const val = Math.random() * 0.5; // Keep small for diagonal dominance
                    triplets.push([i, j, val]);
                }
            }
        }

        const matrix = this.createMatrix(triplets, size, size);
        const b = new Array(size).fill(1.0); // Simple right-hand side

        return { matrix, b };
    }

    /**
     * Benchmark against Python baseline
     * Target: beat 40ms for 1000x1000 matrices (Python baseline)
     */
    benchmark(sizes = [100, 1000]) {
        console.log('🚀 Fast Solver Benchmark - Targeting Python performance');
        console.log('=' * 60);

        const results = [];

        for (const size of sizes) {
            console.log(`\n📊 Testing ${size}x${size} matrix...`);

            const { matrix, b } = this.generateTestMatrix(size, 0.001);

            // Warm up
            this.solve(matrix, b);

            // Benchmark
            const startTime = process.hrtime.bigint();
            const result = this.solve(matrix, b);
            const endTime = process.hrtime.bigint();

            const timeMs = Number(endTime - startTime) / 1e6;

            // Python baseline times (from performance analysis)
            const pythonBaseline = size === 100 ? 2 : (size === 1000 ? 40 : 200);
            const speedup = pythonBaseline / timeMs;

            const status = speedup > 1 ? '✅ FASTER' : '❌ SLOWER';

            console.log(`  Time: ${timeMs.toFixed(2)}ms`);
            console.log(`  Python baseline: ${pythonBaseline}ms`);
            console.log(`  Speedup: ${speedup.toFixed(2)}x ${status}`);
            console.log(`  NNZ: ${matrix.nnz}`);
            console.log(`  Method: ${result.method}`);

            results.push({
                size,
                timeMs,
                pythonBaseline,
                speedup,
                nnz: matrix.nnz,
                method: result.method
            });
        }

        return results;
    }
}

export {
    FastCSRMatrix,
    FastConjugateGradient,
    VectorPool,
    FastSolver
};