#!/usr/bin/env node

/**
 * Test and benchmark the fast solver implementation
 * Goal: Beat Python benchmarks that show MCP Dense is 190x slower
 */

import { FastSolver, FastCSRMatrix } from './js/fast-solver.js';

function testBasicSolver() {
    console.log('🧪 Testing Fast Solver Basic Functionality...\n');

    // Create a simple 2x2 test matrix
    const triplets = [
        [0, 0, 4.0], [0, 1, 1.0],
        [1, 0, 1.0], [1, 1, 3.0]
    ];

    const matrix = FastCSRMatrix.fromTriplets(triplets, 2, 2);
    const b = [1.0, 2.0];

    const solver = new FastSolver();
    const result = solver.solve(matrix, b);

    console.log('Input matrix (2x2):');
    console.log('  [4.0, 1.0]');
    console.log('  [1.0, 3.0]');
    console.log(`Right-hand side: [${b.join(', ')}]`);
    console.log(`Solution: [${result.solution.map(x => x.toFixed(6)).join(', ')}]`);
    console.log(`Execution time: ${result.executionTime.toFixed(3)}ms`);
    console.log(`Method: ${result.method}`);

    // Verify solution
    const y = new Float64Array(2);
    matrix.multiplyVector(result.solution, y);
    const error = Math.sqrt((y[0] - b[0])**2 + (y[1] - b[1])**2);
    console.log(`Verification error: ${error.toFixed(2e-10)}`);
    console.log(error < 1e-8 ? '✅ PASSED' : '❌ FAILED');

    return error < 1e-8;
}

function benchmarkAgainstPython() {
    console.log('\n🏃 Benchmarking Against Python Baselines...\n');

    const solver = new FastSolver();

    // Test the critical sizes from the performance analysis
    const results = solver.benchmark([100, 1000]);

    console.log('\n📈 Summary Results:');
    console.log('Size\tTime(ms)\tPython(ms)\tSpeedup\tStatus');
    console.log('-'.repeat(50));

    let totalSpeedup = 0;
    let passedTests = 0;

    for (const result of results) {
        const status = result.speedup > 1.0 ? '✅ WIN' : '❌ LOSE';
        console.log(`${result.size}\t${result.timeMs.toFixed(1)}\t\t${result.pythonBaseline}\t\t${result.speedup.toFixed(1)}x\t${status}`);

        totalSpeedup += result.speedup;
        if (result.speedup > 1.0) passedTests++;
    }

    const avgSpeedup = totalSpeedup / results.length;
    console.log(`\nAverage speedup: ${avgSpeedup.toFixed(2)}x`);
    console.log(`Tests passed: ${passedTests}/${results.length}`);

    return { results, avgSpeedup, passedTests };
}

function testMemoryEfficiency() {
    console.log('\n💾 Testing Memory Efficiency...\n');

    const solver = new FastSolver();
    const startMemory = process.memoryUsage().heapUsed;

    // Test with 10K matrix (should use < 1MB according to targets)
    console.log('Creating 10,000x10,000 sparse matrix...');
    const { matrix, b } = solver.generateTestMatrix(10000, 0.0001); // Very sparse

    const afterMatrixMemory = process.memoryUsage().heapUsed;
    const matrixMemory = (afterMatrixMemory - startMemory) / 1024 / 1024; // MB

    console.log(`Matrix memory usage: ${matrixMemory.toFixed(2)} MB`);
    console.log(`Target: < 1 MB`);
    console.log(`NNZ: ${matrix.nnz.toLocaleString()}`);
    console.log(`Sparsity: ${(matrix.nnz / (10000 * 10000) * 100).toFixed(4)}%`);

    // Test solve
    console.log('\nSolving 10Kx10K system...');
    const startTime = process.hrtime.bigint();
    const result = solver.solve(matrix, b);
    const endTime = process.hrtime.bigint();

    const solveTime = Number(endTime - startTime) / 1e6;
    const finalMemory = process.memoryUsage().heapUsed;
    const totalMemory = (finalMemory - startMemory) / 1024 / 1024;

    console.log(`Solve time: ${solveTime.toFixed(1)}ms`);
    console.log(`Total memory: ${totalMemory.toFixed(2)} MB`);
    console.log(`Memory target: < 1 MB - ${totalMemory < 1.0 ? '✅ PASSED' : '❌ FAILED'}`);

    return { matrixMemory, totalMemory, solveTime, passed: totalMemory < 1.0 };
}

function testTargetPerformance() {
    console.log('\n🎯 Testing Target Performance Metrics...\n');

    const solver = new FastSolver();

    // Target: 100K×100K system solutions in < 150ms
    console.log('Testing 100K×100K performance target...');
    const { matrix, b } = solver.generateTestMatrix(100000, 0.00001); // Ultra sparse

    console.log(`Matrix size: ${matrix.rows}x${matrix.cols}`);
    console.log(`NNZ: ${matrix.nnz.toLocaleString()}`);
    console.log(`Sparsity: ${(matrix.nnz / (100000 * 100000) * 100).toFixed(6)}%`);

    const startTime = process.hrtime.bigint();
    const result = solver.solve(matrix, b);
    const endTime = process.hrtime.bigint();

    const timeMs = Number(endTime - startTime) / 1e6;
    const target = 150; // ms

    console.log(`Execution time: ${timeMs.toFixed(1)}ms`);
    console.log(`Target: < ${target}ms`);
    console.log(`Status: ${timeMs < target ? '✅ PASSED' : '❌ FAILED'}`);
    console.log(`Method: ${result.method}`);

    return { timeMs, target, passed: timeMs < target };
}

async function main() {
    console.log('🚀 Fast Solver Performance Validation');
    console.log('Targeting Python benchmark improvements');
    console.log('=' * 60);

    const results = {
        basic: false,
        benchmark: { avgSpeedup: 0, passedTests: 0 },
        memory: { passed: false },
        target: { passed: false }
    };

    try {
        // Basic functionality test
        results.basic = testBasicSolver();

        // Benchmark against Python
        const benchmarkResult = benchmarkAgainstPython();
        results.benchmark = benchmarkResult;

        // Memory efficiency test
        const memoryResult = testMemoryEfficiency();
        results.memory = memoryResult;

        // Target performance test
        const targetResult = testTargetPerformance();
        results.target = targetResult;

        // Summary
        console.log('\n🏆 FINAL RESULTS');
        console.log('=' * 60);
        console.log(`Basic functionality: ${results.basic ? '✅ PASS' : '❌ FAIL'}`);
        console.log(`Python benchmark: ${results.benchmark.avgSpeedup.toFixed(2)}x speedup (${results.benchmark.passedTests}/2 tests passed)`);
        console.log(`Memory efficiency: ${results.memory.passed ? '✅ PASS' : '❌ FAIL'}`);
        console.log(`Target performance: ${results.target.passed ? '✅ PASS' : '❌ FAIL'}`);

        const overallScore = (
            (results.basic ? 25 : 0) +
            (results.benchmark.passedTests * 12.5) +
            (results.memory.passed ? 25 : 0) +
            (results.target.passed ? 25 : 0)
        );

        console.log(`\nOverall Score: ${overallScore}/100`);

        if (overallScore >= 75) {
            console.log('🎉 EXCELLENT: Ready for production deployment!');
        } else if (overallScore >= 50) {
            console.log('⚠️  GOOD: Some optimizations still needed');
        } else {
            console.log('❌ NEEDS WORK: Significant performance improvements required');
        }

    } catch (error) {
        console.error('❌ Test failed with error:', error.message);
        console.error(error.stack);
        process.exit(1);
    }
}

main();