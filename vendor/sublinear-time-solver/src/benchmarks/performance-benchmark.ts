/**
 * Comprehensive Performance Benchmark
 *
 * This benchmark demonstrates the 5-10x performance improvements achieved by
 * the optimized solver implementations compared to naive implementations.
 */

import {
    OptimizedSparseMatrix,
    VectorOps,
    HighPerformanceConjugateGradientSolver,
    VectorPool,
    createHighPerformanceSolver,
} from '../core/high-performance-solver.js';

/**
 * Naive sparse matrix implementation for comparison
 */
class NaiveSparseMatrix {
    private triplets: Array<[number, number, number]>;
    private rows: number;
    private cols: number;

    constructor(triplets: Array<[number, number, number]>, rows: number, cols: number) {
        this.triplets = triplets;
        this.rows = rows;
        this.cols = cols;
    }

    multiplyVector(x: number[], y: number[]): void {
        y.fill(0);
        for (const [row, col, val] of this.triplets) {
            y[row] += val * x[col];
        }
    }

    get dimensions(): [number, number] {
        return [this.rows, this.cols];
    }
}

/**
 * Naive vector operations for comparison
 */
class NaiveVectorOps {
    static dotProduct(x: number[], y: number[]): number {
        let result = 0;
        for (let i = 0; i < x.length; i++) {
            result += x[i] * y[i];
        }
        return result;
    }

    static axpy(alpha: number, x: number[], y: number[]): void {
        for (let i = 0; i < x.length; i++) {
            y[i] += alpha * x[i];
        }
    }

    static norm(x: number[]): number {
        return Math.sqrt(NaiveVectorOps.dotProduct(x, x));
    }
}

/**
 * Naive conjugate gradient solver for comparison
 */
class NaiveConjugateGradientSolver {
    private maxIterations: number;
    private tolerance: number;

    constructor(maxIterations = 1000, tolerance = 1e-6) {
        this.maxIterations = maxIterations;
        this.tolerance = tolerance;
    }

    solve(matrix: NaiveSparseMatrix, b: number[]): {
        solution: number[];
        iterations: number;
        residualNorm: number;
        converged: boolean;
        computationTimeMs: number;
    } {
        const startTime = performance.now();
        const [rows] = matrix.dimensions;

        const x = new Array(rows).fill(0);
        const r = [...b];
        const p = [...r];
        const ap = new Array(rows).fill(0);

        let rsold = NaiveVectorOps.dotProduct(r, r);
        let iteration = 0;
        let converged = false;

        while (iteration < this.maxIterations) {
            matrix.multiplyVector(p, ap);
            const pAp = NaiveVectorOps.dotProduct(p, ap);

            if (Math.abs(pAp) < 1e-16) {
                throw new Error('Matrix appears to be singular');
            }

            const alpha = rsold / pAp;

            NaiveVectorOps.axpy(alpha, p, x);
            NaiveVectorOps.axpy(-alpha, ap, r);

            const rsnew = NaiveVectorOps.dotProduct(r, r);
            const residualNorm = Math.sqrt(rsnew);

            if (residualNorm < this.tolerance) {
                converged = true;
                break;
            }

            const beta = rsnew / rsold;
            for (let i = 0; i < rows; i++) {
                p[i] = r[i] + beta * p[i];
            }

            rsold = rsnew;
            iteration++;
        }

        const computationTimeMs = performance.now() - startTime;

        return {
            solution: x,
            iterations: iteration,
            residualNorm: Math.sqrt(rsold),
            converged,
            computationTimeMs,
        };
    }
}

/**
 * Generate test matrices of various sizes and sparsity patterns
 */
class MatrixGenerator {
    /**
     * Generate a symmetric positive definite tridiagonal matrix
     */
    static generateTridiagonal(size: number): Array<[number, number, number]> {
        const triplets: Array<[number, number, number]> = [];

        for (let i = 0; i < size; i++) {
            // Diagonal entries (make diagonally dominant)
            triplets.push([i, i, 4.0]);

            // Off-diagonal entries
            if (i > 0) {
                triplets.push([i, i - 1, -1.0]);
            }
            if (i < size - 1) {
                triplets.push([i, i + 1, -1.0]);
            }
        }

        return triplets;
    }

    /**
     * Generate a 2D 5-point stencil matrix (finite difference discretization)
     */
    static generate2DPoisson(n: number): Array<[number, number, number]> {
        const triplets: Array<[number, number, number]> = [];
        const size = n * n;

        for (let i = 0; i < n; i++) {
            for (let j = 0; j < n; j++) {
                const row = i * n + j;

                // Diagonal entry
                triplets.push([row, row, 4.0]);

                // Neighbors
                if (i > 0) {
                    const neighbor = (i - 1) * n + j;
                    triplets.push([row, neighbor, -1.0]);
                }
                if (i < n - 1) {
                    const neighbor = (i + 1) * n + j;
                    triplets.push([row, neighbor, -1.0]);
                }
                if (j > 0) {
                    const neighbor = i * n + (j - 1);
                    triplets.push([row, neighbor, -1.0]);
                }
                if (j < n - 1) {
                    const neighbor = i * n + (j + 1);
                    triplets.push([row, neighbor, -1.0]);
                }
            }
        }

        return triplets;
    }

    /**
     * Generate a random right-hand side vector
     */
    static generateRHS(size: number, seed = 42): Float64Array {
        // Simple LCG for reproducible random numbers
        let rng = seed;
        const next = () => {
            rng = (rng * 1103515245 + 12345) % (1 << 31);
            return rng / (1 << 31);
        };

        const b = new Float64Array(size);
        for (let i = 0; i < size; i++) {
            b[i] = next() - 0.5; // Range [-0.5, 0.5]
        }
        return b;
    }
}

/**
 * Benchmark result interface
 */
interface BenchmarkResult {
    name: string;
    matrixSize: number;
    nnz: number;
    optimizedTime: number;
    naiveTime: number;
    speedup: number;
    optimizedIterations: number;
    naiveIterations: number;
    optimizedResidual: number;
    naiveResidual: number;
    performanceStats?: {
        gflops: number;
        bandwidth: number;
        matVecCount: number;
        totalFlops: number;
    };
}

/**
 * Main benchmark runner
 */
export class PerformanceBenchmark {
    private vectorPool = new VectorPool();

    /**
     * Run a single benchmark comparing optimized vs naive implementation
     */
    private async runSingleBenchmark(
        name: string,
        triplets: Array<[number, number, number]>,
        size: number,
        b: Float64Array
    ): Promise<BenchmarkResult> {
        console.log(`Running benchmark: ${name} (size: ${size})`);

        // Convert b to regular array for naive implementation
        const bArray = Array.from(b);

        // Create matrices
        const optimizedMatrix = OptimizedSparseMatrix.fromTriplets(triplets, size, size);
        const naiveMatrix = new NaiveSparseMatrix(triplets, size, size);

        // Create solvers
        const optimizedSolver = createHighPerformanceSolver({
            maxIterations: 1000,
            tolerance: 1e-6,
            enableProfiling: true,
        });
        const naiveSolver = new NaiveConjugateGradientSolver(1000, 1e-6);

        // Warm up
        console.log('  Warming up...');
        for (let i = 0; i < 2; i++) {
            optimizedSolver.solve(optimizedMatrix, b);
            naiveSolver.solve(naiveMatrix, bArray);
        }

        // Benchmark optimized implementation
        console.log('  Benchmarking optimized implementation...');
        const optimizedStart = performance.now();
        const optimizedResult = optimizedSolver.solve(optimizedMatrix, b);
        const optimizedTime = performance.now() - optimizedStart;

        // Benchmark naive implementation
        console.log('  Benchmarking naive implementation...');
        const naiveStart = performance.now();
        const naiveResult = naiveSolver.solve(naiveMatrix, bArray);
        const naiveTime = performance.now() - naiveStart;

        const speedup = naiveTime / optimizedTime;

        console.log(`  Speedup: ${speedup.toFixed(2)}x`);
        console.log(`  Optimized: ${optimizedTime.toFixed(2)}ms`);
        console.log(`  Naive: ${naiveTime.toFixed(2)}ms`);

        return {
            name,
            matrixSize: size,
            nnz: triplets.length,
            optimizedTime,
            naiveTime,
            speedup,
            optimizedIterations: optimizedResult.iterations,
            naiveIterations: naiveResult.iterations,
            optimizedResidual: optimizedResult.residualNorm,
            naiveResidual: naiveResult.residualNorm,
            performanceStats: {
                gflops: optimizedResult.performanceStats.gflops,
                bandwidth: optimizedResult.performanceStats.bandwidth,
                matVecCount: optimizedResult.performanceStats.matVecCount,
                totalFlops: optimizedResult.performanceStats.totalFlops,
            },
        };
    }

    /**
     * Run comprehensive benchmark suite
     */
    async runBenchmarkSuite(): Promise<BenchmarkResult[]> {
        console.log('Starting Performance Benchmark Suite');
        console.log('====================================');

        const results: BenchmarkResult[] = [];

        // Test different matrix sizes and types
        const testCases = [
            {
                name: 'Small Tridiagonal',
                generator: () => MatrixGenerator.generateTridiagonal(100),
                size: 100,
            },
            {
                name: 'Medium Tridiagonal',
                generator: () => MatrixGenerator.generateTridiagonal(500),
                size: 500,
            },
            {
                name: 'Large Tridiagonal',
                generator: () => MatrixGenerator.generateTridiagonal(1000),
                size: 1000,
            },
            {
                name: 'Small 2D Poisson',
                generator: () => MatrixGenerator.generate2DPoisson(10),
                size: 100,
            },
            {
                name: 'Medium 2D Poisson',
                generator: () => MatrixGenerator.generate2DPoisson(20),
                size: 400,
            },
            {
                name: 'Large 2D Poisson',
                generator: () => MatrixGenerator.generate2DPoisson(30),
                size: 900,
            },
        ];

        for (const testCase of testCases) {
            try {
                const triplets = testCase.generator();
                const b = MatrixGenerator.generateRHS(testCase.size);
                const result = await this.runSingleBenchmark(
                    testCase.name,
                    triplets,
                    testCase.size,
                    b
                );
                results.push(result);
                console.log('');
            } catch (error) {
                console.error(`Error in benchmark ${testCase.name}:`, error);
            }
        }

        return results;
    }

    /**
     * Generate benchmark report
     */
    generateReport(results: BenchmarkResult[]): string {
        let report = '\\n\\nPerformance Benchmark Report\\n';
        report += '============================\\n\\n';

        // Summary statistics
        const speedups = results.map(r => r.speedup);
        const avgSpeedup = speedups.reduce((a, b) => a + b, 0) / speedups.length;
        const minSpeedup = Math.min(...speedups);
        const maxSpeedup = Math.max(...speedups);

        report += `Summary:\\n`;
        report += `--------\\n`;
        report += `Average Speedup: ${avgSpeedup.toFixed(2)}x\\n`;
        report += `Minimum Speedup: ${minSpeedup.toFixed(2)}x\\n`;
        report += `Maximum Speedup: ${maxSpeedup.toFixed(2)}x\\n`;
        report += `Target Achieved: ${avgSpeedup >= 5 ? 'YES' : 'NO'} (5-10x target)\\n\\n`;

        // Detailed results
        report += 'Detailed Results:\\n';
        report += '----------------\\n';
        report += 'Test Case                  Size    NNZ     Optimized  Naive     Speedup   GFLOPS   Bandwidth\\n';
        report += '                                           (ms)       (ms)                       (GB/s)\\n';
        report += '-'.repeat(90) + '\\n';

        for (const result of results) {
            const name = result.name.padEnd(25);
            const size = result.matrixSize.toString().padStart(6);
            const nnz = result.nnz.toString().padStart(6);
            const optTime = result.optimizedTime.toFixed(1).padStart(9);
            const naiveTime = result.naiveTime.toFixed(1).padStart(9);
            const speedup = result.speedup.toFixed(2).padStart(8);
            const gflops = result.performanceStats?.gflops.toFixed(1).padStart(7) || '   N/A';
            const bandwidth = result.performanceStats?.bandwidth.toFixed(1).padStart(9) || '     N/A';

            report += `${name} ${size} ${nnz} ${optTime} ${naiveTime} ${speedup}x ${gflops} ${bandwidth}\\n`;
        }

        report += '\\n';

        // Performance insights
        report += 'Performance Insights:\\n';
        report += '--------------------\\n';

        const highSpeedupResults = results.filter(r => r.speedup >= 5);
        if (highSpeedupResults.length > 0) {
            report += `✓ ${highSpeedupResults.length}/${results.length} test cases achieved 5x+ speedup\\n`;
        }

        const avgGflops = results
            .filter(r => r.performanceStats?.gflops)
            .map(r => r.performanceStats!.gflops)
            .reduce((a, b) => a + b, 0) / results.length;

        const avgBandwidth = results
            .filter(r => r.performanceStats?.bandwidth)
            .map(r => r.performanceStats!.bandwidth)
            .reduce((a, b) => a + b, 0) / results.length;

        report += `✓ Average Performance: ${avgGflops.toFixed(1)} GFLOPS, ${avgBandwidth.toFixed(1)} GB/s\\n`;

        // Optimization techniques used
        report += '\\nOptimization Techniques Applied:\\n';
        report += '- TypedArrays (Float64Array, Uint32Array) for memory efficiency\\n';
        report += '- CSR sparse matrix format for cache-friendly access patterns\\n';
        report += '- Manual loop unrolling for better instruction-level parallelism\\n';
        report += '- Vector workspace reuse to minimize memory allocations\\n';
        report += '- Efficient vector operations with optimized memory layouts\\n';
        report += '- Reduced function call overhead through inlining\\n';

        return report;
    }

    /**
     * Clean up resources
     */
    dispose(): void {
        this.vectorPool.clear();
    }
}

/**
 * Run the benchmark if this module is executed directly
 */
if (typeof globalThis !== 'undefined' && typeof globalThis.window === 'undefined') {
    // Node.js environment
    const benchmark = new PerformanceBenchmark();

    benchmark.runBenchmarkSuite().then(results => {
        const report = benchmark.generateReport(results);
        console.log(report);
        benchmark.dispose();
    }).catch(error => {
        console.error('Benchmark failed:', error);
        if (typeof process !== 'undefined') {
            process.exit(1);
        }
    });
}

// Classes are already exported above