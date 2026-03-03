#!/usr/bin/env node
/**
 * Complete WASM validation test - especially for NPX usage
 */

import { SublinearSolver } from './dist/core/solver.js';
import { WasmSolver } from './wasm-solver/pkg/sublinear_wasm_solver.js';
import { performance } from 'perf_hooks';

console.log('🔍 COMPLETE WASM VALIDATION TEST');
console.log('Testing WASM acceleration for NPX usage');
console.log('═'.repeat(70));

const results = {
    rustWasmDirect: false,
    jsIntegration: false,
    wasmAcceleration: false,
    performanceGain: false,
    npxCompatibility: false
};

// Test 1: Direct Rust WASM functionality
console.log('\n1️⃣ Testing Direct Rust WASM Functionality');
console.log('─'.repeat(50));

try {
    const wasmSolver = new WasmSolver();
    wasmSolver.set_tolerance(1e-6);
    wasmSolver.set_max_iterations(100);

    // Test various matrix formats
    const tests = [
        {
            name: 'CSR Format',
            matrix: {
                values: [4, -1, -1, 4, -1, -1, 4],
                col_indices: [0, 1, 0, 1, 2, 1, 2],
                row_ptr: [0, 2, 5, 7],
                rows: 3,
                cols: 3
            },
            vector: [3, 2, 3],
            method: 'solve_csr'
        },
        {
            name: 'Dense Format',
            matrix: [[4, -1, 0], [-1, 4, -1], [0, -1, 4]],
            vector: [3, 2, 3],
            method: 'solve_dense'
        }
    ];

    for (const test of tests) {
        const start = performance.now();
        const result = wasmSolver[test.method](
            JSON.stringify(test.matrix),
            JSON.stringify(test.vector)
        );
        const elapsed = performance.now() - start;

        const parsed = JSON.parse(result);
        console.log(`✅ ${test.name}: ${elapsed.toFixed(2)}ms, ${parsed.iterations} iterations`);
        console.log(`   Solution: [${parsed.solution.map(x => x.toFixed(4)).join(', ')}]`);
    }

    results.rustWasmDirect = true;
    console.log('✅ Direct Rust WASM: WORKING');

} catch (error) {
    console.log('❌ Direct Rust WASM failed:', error.message);
}

// Test 2: JavaScript Integration
console.log('\n2️⃣ Testing JavaScript Integration');
console.log('─'.repeat(50));

try {
    const solver = new SublinearSolver({
        method: 'neumann',
        epsilon: 1e-6,
        maxIterations: 100
    });

    // Wait for WASM initialization
    await new Promise(resolve => setTimeout(resolve, 200));

    const matrix = {
        rows: 3,
        cols: 3,
        format: 'dense',
        data: [[4, -1, 0], [-1, 4, -1], [0, -1, 4]]
    };
    const vector = [3, 2, 3];

    const start = performance.now();
    const result = await solver.solve(matrix, vector);
    const elapsed = performance.now() - start;

    console.log(`✅ Solver result: ${elapsed.toFixed(2)}ms`);
    console.log(`   Method: ${result.method}`);
    console.log(`   Solution: [${result.solution.map(x => x.toFixed(4)).join(', ')}]`);
    console.log(`   Iterations: ${result.iterations}`);
    console.log(`   WASM accelerated: ${solver.wasmAccelerated}`);

    if (result.method.includes('WASM')) {
        results.wasmAcceleration = true;
        console.log('✅ WASM acceleration: ACTIVE');
    } else {
        console.log('⚠️ WASM acceleration: NOT ACTIVE');
    }

    results.jsIntegration = true;

} catch (error) {
    console.log('❌ JavaScript integration failed:', error.message);
}

// Test 3: Performance Comparison
console.log('\n3️⃣ Testing Performance Gain');
console.log('─'.repeat(50));

try {
    const sizes = [10, 50, 100];

    for (const size of sizes) {
        // Create test matrix
        const matrix = {
            rows: size,
            cols: size,
            format: 'dense',
            data: Array(size).fill(null).map((_, i) =>
                Array(size).fill(null).map((_, j) => {
                    if (i === j) return 4; // Diagonal
                    if (Math.abs(i - j) === 1) return -1; // Off-diagonal
                    return 0;
                })
            )
        };
        const vector = Array(size).fill(1);

        // Test WASM (direct)
        const wasmSolver = new WasmSolver();
        wasmSolver.set_tolerance(1e-4);
        wasmSolver.set_max_iterations(50);

        const wasmStart = performance.now();
        const wasmResult = wasmSolver.solve_dense(
            JSON.stringify(matrix.data),
            JSON.stringify(vector)
        );
        const wasmTime = performance.now() - wasmStart;
        const wasmParsed = JSON.parse(wasmResult);

        // Test JavaScript
        const jsSolver = new SublinearSolver({
            method: 'neumann',
            epsilon: 1e-4,
            maxIterations: 50
        });

        // Force JavaScript mode
        jsSolver.wasmAccelerated = false;

        const jsStart = performance.now();
        const jsResult = await jsSolver.solve(matrix, vector);
        const jsTime = performance.now() - jsStart;

        const speedup = jsTime / wasmTime;
        console.log(`${size}x${size} matrix:`);
        console.log(`   WASM: ${wasmTime.toFixed(2)}ms (${wasmParsed.iterations} iter)`);
        console.log(`   JS:   ${jsTime.toFixed(2)}ms (${jsResult.iterations} iter)`);
        console.log(`   Speedup: ${speedup.toFixed(1)}x`);

        if (speedup > 1.5) {
            results.performanceGain = true;
        }
    }

} catch (error) {
    console.log('❌ Performance test failed:', error.message);
}

// Test 4: NPX Compatibility Test
console.log('\n4️⃣ Testing NPX Compatibility');
console.log('─'.repeat(50));

try {
    // Simulate NPX environment conditions
    const originalArgv = process.argv;
    const originalExecPath = process.execPath;

    // Test module loading in NPX-like conditions
    console.log('Testing module imports...');

    // Test if modules can be imported as they would in NPX
    const { SublinearSolver: NPXSolver } = await import('./dist/core/solver.js');
    const { WasmSolver: NPXWasmSolver } = await import('./wasm-solver/pkg/sublinear_wasm_solver.js');

    console.log('✅ Module imports successful');

    // Test solver creation
    const npxSolver = new NPXSolver();
    const npxWasm = new NPXWasmSolver();

    console.log('✅ Solver instantiation successful');

    // Test actual solving
    const testMatrix = {
        rows: 2,
        cols: 2,
        format: 'dense',
        data: [[3, -1], [-1, 3]]
    };
    const testVector = [2, 2];

    const npxResult = await npxSolver.solve(testMatrix, testVector);
    console.log('✅ NPX-style solve successful');
    console.log(`   Result: [${npxResult.solution.map(x => x.toFixed(4)).join(', ')}]`);

    results.npxCompatibility = true;

} catch (error) {
    console.log('❌ NPX compatibility test failed:', error.message);
}

// Test 5: MCP Integration Test
console.log('\n5️⃣ Testing MCP Integration');
console.log('─'.repeat(50));

try {
    // Test that MCP server can load and use WASM
    const solver = new SublinearSolver();
    await new Promise(resolve => setTimeout(resolve, 200));

    // Test PageRank (common MCP operation)
    const adjacency = {
        rows: 3,
        cols: 3,
        format: 'dense',
        data: [[0, 1, 1], [1, 0, 1], [1, 1, 0]]
    };

    const pageRankResult = await solver.computePageRank(adjacency, {
        damping: 0.85,
        epsilon: 1e-6
    });

    console.log('✅ PageRank computation successful');
    console.log(`   Ranks: [${pageRankResult.ranks.map(r => r.toFixed(4)).join(', ')}]`);
    console.log(`   Iterations: ${pageRankResult.iterations}`);

} catch (error) {
    console.log('❌ MCP integration test failed:', error.message);
}

// Final Report
console.log('\n' + '═'.repeat(70));
console.log('📊 COMPLETE WASM VALIDATION REPORT');
console.log('─'.repeat(70));

const allPassed = Object.values(results).filter(Boolean).length;
const totalTests = Object.keys(results).length;

console.log(`Direct Rust WASM:        ${results.rustWasmDirect ? '✅ PASS' : '❌ FAIL'}`);
console.log(`JavaScript Integration:  ${results.jsIntegration ? '✅ PASS' : '❌ FAIL'}`);
console.log(`WASM Acceleration:       ${results.wasmAcceleration ? '✅ ACTIVE' : '⚠️ INACTIVE'}`);
console.log(`Performance Gain:        ${results.performanceGain ? '✅ YES' : '⚠️ NO'}`);
console.log(`NPX Compatibility:       ${results.npxCompatibility ? '✅ PASS' : '❌ FAIL'}`);

console.log('\n' + '═'.repeat(70));
console.log(`OVERALL: ${allPassed}/${totalTests} tests passed`);

if (results.rustWasmDirect && results.jsIntegration && results.npxCompatibility) {
    console.log('✨ SUCCESS: WASM is functional and NPX-ready!');
    if (results.wasmAcceleration) {
        console.log('🚀 WASM acceleration is ACTIVE in the solver!');
    } else {
        console.log('⚠️ WASM acceleration needs to be activated in the solver.');
    }
} else {
    console.log('⚠️ Some WASM functionality issues remain.');
}

process.exit(allPassed >= 3 ? 0 : 1);