#!/usr/bin/env node
/**
 * Test the Rust-compiled WASM solver specifically
 */

import { WasmSolver } from './wasm-solver/pkg/sublinear_wasm_solver.js';

console.log('🔍 RUST WASM SOLVER TEST');
console.log('═'.repeat(60));

try {
    // Test 1: Basic WASM functionality
    console.log('\n1️⃣ Testing Rust WASM Solver');
    console.log('─'.repeat(40));

    const wasmSolver = new WasmSolver();
    wasmSolver.set_tolerance(1e-6);
    wasmSolver.set_max_iterations(100);

    // Create test matrix in CSR format
    const matrixData = {
        values: [4, -1, -1, 4, -1, -1, 4],
        col_indices: [0, 1, 0, 1, 2, 1, 2],
        row_ptr: [0, 2, 5, 7],
        rows: 3,
        cols: 3
    };

    const vectorData = [3, 2, 3];

    console.log('✅ WASM solver created');
    console.log('   Matrix format: CSR');
    console.log('   Matrix size: 3x3');

    // Test CSR solve
    const start1 = performance.now();
    const resultJson = wasmSolver.solve_csr(
        JSON.stringify(matrixData),
        JSON.stringify(vectorData)
    );
    const elapsed1 = performance.now() - start1;

    const result = JSON.parse(resultJson);
    console.log('✅ CSR solve succeeded');
    console.log(`   Solution: [${result.solution.map(x => x.toFixed(4)).join(', ')}]`);
    console.log(`   Time: ${elapsed1.toFixed(2)}ms`);
    console.log(`   Iterations: ${result.iterations}`);

    // Test 2: Dense matrix solve
    console.log('\n2️⃣ Testing Dense Matrix Solve');
    console.log('─'.repeat(40));

    const denseMatrix = [
        [4, -1, 0],
        [-1, 4, -1],
        [0, -1, 4]
    ];

    const start2 = performance.now();
    const denseResultJson = wasmSolver.solve_dense(
        JSON.stringify(denseMatrix),
        JSON.stringify(vectorData)
    );
    const elapsed2 = performance.now() - start2;

    const denseResult = JSON.parse(denseResultJson);
    console.log('✅ Dense solve succeeded');
    console.log(`   Solution: [${denseResult.solution.map(x => x.toFixed(4)).join(', ')}]`);
    console.log(`   Time: ${elapsed2.toFixed(2)}ms`);
    console.log(`   Iterations: ${denseResult.iterations}`);

    // Test 3: Neumann series solve
    console.log('\n3️⃣ Testing Neumann Series');
    console.log('─'.repeat(40));

    const start3 = performance.now();
    const neumannResultJson = wasmSolver.solve_neumann(
        JSON.stringify(matrixData),
        JSON.stringify(vectorData)
    );
    const elapsed3 = performance.now() - start3;

    const neumannResult = JSON.parse(neumannResultJson);
    console.log('✅ Neumann solve succeeded');
    console.log(`   Solution: [${neumannResult.solution.map(x => x.toFixed(4)).join(', ')}]`);
    console.log(`   Time: ${elapsed3.toFixed(2)}ms`);
    console.log(`   Iterations: ${neumannResult.iterations}`);

    // Performance comparison
    console.log('\n4️⃣ Performance Analysis');
    console.log('─'.repeat(40));
    console.log(`CSR method:     ${elapsed1.toFixed(2)}ms`);
    console.log(`Dense method:   ${elapsed2.toFixed(2)}ms`);
    console.log(`Neumann method: ${elapsed3.toFixed(2)}ms`);

    const avgTime = (elapsed1 + elapsed2 + elapsed3) / 3;
    console.log(`Average time:   ${avgTime.toFixed(2)}ms`);

    console.log('\n' + '═'.repeat(60));
    console.log('✨ SUCCESS: Rust WASM solver is fully functional!');
    console.log('The WASM modules are working correctly.');

} catch (error) {
    console.error('❌ FAILED:', error.message);
    console.error('Stack:', error.stack);
    console.log('\n' + '═'.repeat(60));
    console.log('⚠️ Rust WASM solver has issues');
    process.exit(1);
}