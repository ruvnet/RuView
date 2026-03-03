#!/usr/bin/env node
/**
 * Simple test of WASM solver
 */

const {
    WasmSolver,
    create_test_matrix,
    create_test_vector,
    version
} = require('./wasm-solver/pkg/sublinear_wasm_solver.js');

console.log('Testing WASM Solver...\n');

try {
    // Create solver
    const solver = new WasmSolver();
    console.log(`✅ Solver created, version: ${version()}`);

    // Test with generated matrix
    console.log('\nTesting with generated matrix:');
    const matrixJson = create_test_matrix(3);
    const vectorJson = create_test_vector(3);

    console.log('Matrix JSON:', matrixJson);
    console.log('Vector JSON:', vectorJson);

    try {
        const resultJson = solver.solve_csr(matrixJson, vectorJson);
        const result = JSON.parse(resultJson);
        console.log('✅ CSR solve succeeded!');
        console.log('Result:', result);
    } catch (e) {
        console.error('❌ CSR solve failed:', e.message);
    }

    // Try Neumann method
    try {
        const resultJson = solver.solve_neumann(matrixJson, vectorJson);
        const result = JSON.parse(resultJson);
        console.log('✅ Neumann solve succeeded!');
        console.log('Result:', result);
    } catch (e) {
        console.error('❌ Neumann solve failed:', e.message);
    }

} catch (error) {
    console.error('Fatal error:', error);
}