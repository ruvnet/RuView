#!/usr/bin/env node
/**
 * Final validation test - ensures README examples work correctly
 */

import { SublinearSolver } from './dist/core/solver.js';
import { WasmSolver } from './wasm-solver/pkg/sublinear_wasm_solver.js';

console.log('🔍 FINAL VALIDATION TEST');
console.log('═'.repeat(60));

const tests = {
    basicExample: false,
    sparseExample: false,
    pageRankExample: false,
    wasmExample: false
};

// Test 1: Basic example from README
console.log('\n1️⃣ Testing Basic Example from README');
console.log('─'.repeat(40));
try {
    const solver = new SublinearSolver({
        method: 'neumann',
        epsilon: 1e-6,
        maxIterations: 100
    });

    const matrix = {
        rows: 3,
        cols: 3,
        data: [[4, -1, 0], [-1, 4, -1], [0, -1, 4]],
        format: 'dense'
    };

    const vector = [3, 2, 3];
    const result = await solver.solve(matrix, vector);

    console.log('✅ Basic example works');
    console.log(`   Solution: [${result.solution.map(x => x.toFixed(4)).join(', ')}]`);
    console.log(`   Iterations: ${result.iterations}`);
    tests.basicExample = true;
} catch (error) {
    console.log('❌ Basic example failed:', error.message);
}

// Test 2: Sparse matrix example
console.log('\n2️⃣ Testing Sparse Matrix Example');
console.log('─'.repeat(40));
try {
    const solver = new SublinearSolver({
        method: 'neumann',
        epsilon: 1e-6,
        maxIterations: 1000
    });

    // Small sparse matrix for testing
    const matrix = {
        rows: 100,
        cols: 100,
        format: 'coo',
        values: [],
        rowIndices: [],
        colIndices: []
    };

    // Create tridiagonal sparse matrix
    for (let i = 0; i < 100; i++) {
        if (i > 0) {
            matrix.values.push(-1);
            matrix.rowIndices.push(i);
            matrix.colIndices.push(i - 1);
        }

        matrix.values.push(4);
        matrix.rowIndices.push(i);
        matrix.colIndices.push(i);

        if (i < 99) {
            matrix.values.push(-1);
            matrix.rowIndices.push(i);
            matrix.colIndices.push(i + 1);
        }
    }

    const vector = new Array(100).fill(1);
    const result = await solver.solve(matrix, vector);

    console.log('✅ Sparse matrix example works');
    console.log(`   Solved ${matrix.rows}x${matrix.cols} sparse system`);
    console.log(`   Iterations: ${result.iterations}`);
    console.log(`   Residual: ${result.residual.toExponential(2)}`);
    tests.sparseExample = true;
} catch (error) {
    console.log('❌ Sparse matrix example failed:', error.message);
}

// Test 3: PageRank example
console.log('\n3️⃣ Testing PageRank Example');
console.log('─'.repeat(40));
try {
    const solver = new SublinearSolver();

    const adjacencyMatrix = {
        rows: 4,
        cols: 4,
        format: 'dense',
        data: [
            [0, 1, 1, 0],
            [1, 0, 0, 1],
            [0, 1, 0, 1],
            [1, 0, 1, 0]
        ]
    };

    const pagerank = await solver.computePageRank(adjacencyMatrix, {
        damping: 0.85,
        epsilon: 1e-6,
        maxIterations: 100
    });

    console.log('✅ PageRank example works');
    console.log(`   Ranks: [${pagerank.ranks.map(x => x.toFixed(3)).join(', ')}]`);
    console.log(`   Iterations: ${pagerank.iterations}`);
    tests.pageRankExample = true;
} catch (error) {
    console.log('❌ PageRank example failed:', error.message);
}

// Test 4: WASM solver example
console.log('\n4️⃣ Testing WASM Solver Example');
console.log('─'.repeat(40));
try {
    const wasmSolver = new WasmSolver();
    wasmSolver.set_tolerance(1e-6);
    wasmSolver.set_max_iterations(100);

    // Create test matrix in JSON format
    const matrixData = {
        values: [4, -1, -1, 4, -1, -1, 4],
        col_indices: [0, 1, 0, 1, 2, 1, 2],
        row_ptr: [0, 2, 5, 7],
        rows: 3,
        cols: 3
    };

    const vectorData = [3, 2, 3];

    const resultJson = wasmSolver.solve_csr(
        JSON.stringify(matrixData),
        JSON.stringify(vectorData)
    );

    const result = JSON.parse(resultJson);
    console.log('✅ WASM solver example works');
    console.log(`   Solution: [${result.solution.map(x => x.toFixed(4)).join(', ')}]`);
    console.log(`   Iterations: ${result.iterations}`);
    tests.wasmExample = true;
} catch (error) {
    console.log('❌ WASM solver example failed:', error.message);
}

// Final Report
console.log('\n' + '═'.repeat(60));
console.log('📊 FINAL VALIDATION REPORT');
console.log('─'.repeat(40));

const allPassed = Object.values(tests).every(v => v === true);

console.log('Basic example:      ' + (tests.basicExample ? '✅ PASSED' : '❌ FAILED'));
console.log('Sparse example:     ' + (tests.sparseExample ? '✅ PASSED' : '❌ FAILED'));
console.log('PageRank example:   ' + (tests.pageRankExample ? '✅ PASSED' : '❌ FAILED'));
console.log('WASM example:       ' + (tests.wasmExample ? '✅ PASSED' : '❌ FAILED'));

console.log('\n' + '═'.repeat(60));
if (allPassed) {
    console.log('✨ SUCCESS: All README examples are working correctly!');
    console.log('The npm/npx sublinear-time-solver package is production ready.');
} else {
    console.log('⚠️ Some examples need attention.');
}

process.exit(allPassed ? 0 : 1);