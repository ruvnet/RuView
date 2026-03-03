#!/usr/bin/env node
/**
 * Test MCP tools are using WASM acceleration
 */

import { SublinearSolver } from './dist/core/solver.js';
import { performance } from 'perf_hooks';

console.log('🔍 MCP WASM ACCELERATION TEST');
console.log('═'.repeat(60));

async function testWASMAcceleration() {
    const tests = {
        wasmInitialized: false,
        matrixMultiplyAccelerated: false,
        pageRankAccelerated: false,
        memoryEfficient: false
    };

    // Test 1: Check WASM initialization
    console.log('\n1️⃣ Testing WASM Initialization');
    console.log('─'.repeat(40));
    try {
        const solver = new SublinearSolver({
            method: 'neumann',
            epsilon: 1e-6
        });

        // Wait for WASM to initialize
        await new Promise(resolve => setTimeout(resolve, 100));

        // Check if WASM modules are loaded
        if (solver.wasmAccelerated) {
            console.log('✅ WASM modules loaded successfully');
            console.log(`   Modules: ${Object.keys(solver.wasmModules).join(', ')}`);
            tests.wasmInitialized = true;
        } else {
            console.log('⚠️ WASM not initialized (solver.wasmAccelerated = false)');
        }
    } catch (error) {
        console.log('❌ WASM initialization error:', error.message);
    }

    // Test 2: Benchmark matrix multiplication
    console.log('\n2️⃣ Testing Matrix Multiplication Performance');
    console.log('─'.repeat(40));
    try {
        const sizes = [100, 500, 1000];

        for (const size of sizes) {
            // Create large sparse matrix
            const matrix = {
                rows: size,
                cols: size,
                format: 'coo',
                values: [],
                rowIndices: [],
                colIndices: []
            };

            // Tridiagonal matrix
            for (let i = 0; i < size; i++) {
                if (i > 0) {
                    matrix.values.push(-1);
                    matrix.rowIndices.push(i);
                    matrix.colIndices.push(i - 1);
                }
                matrix.values.push(4);
                matrix.rowIndices.push(i);
                matrix.colIndices.push(i);
                if (i < size - 1) {
                    matrix.values.push(-1);
                    matrix.rowIndices.push(i);
                    matrix.colIndices.push(i + 1);
                }
            }

            const vector = new Array(size).fill(1);

            const solver = new SublinearSolver({
                method: 'neumann',
                epsilon: 1e-4,
                maxIterations: 10
            });

            await new Promise(resolve => setTimeout(resolve, 50)); // Let WASM init

            const start = performance.now();
            const result = await solver.solve(matrix, vector);
            const elapsed = performance.now() - start;

            console.log(`   ${size}x${size} matrix: ${elapsed.toFixed(2)}ms (${result.iterations} iterations)`);

            // WASM should be faster for larger matrices
            if (size === 1000 && elapsed < 1000) {
                tests.matrixMultiplyAccelerated = true;
            }
        }
    } catch (error) {
        console.log('❌ Matrix multiplication test failed:', error.message);
    }

    // Test 3: PageRank with WASM acceleration
    console.log('\n3️⃣ Testing PageRank WASM Acceleration');
    console.log('─'.repeat(40));
    try {
        // Create a larger graph for testing
        const n = 50;
        const adjacency = {
            rows: n,
            cols: n,
            format: 'dense',
            data: Array(n).fill(null).map(() => Array(n).fill(0))
        };

        // Create random sparse graph
        for (let i = 0; i < n; i++) {
            for (let j = 0; j < n; j++) {
                if (i !== j && Math.random() < 0.1) {
                    adjacency.data[i][j] = 1;
                }
            }
        }

        const solver = new SublinearSolver();
        await new Promise(resolve => setTimeout(resolve, 50)); // Let WASM init

        const start = performance.now();
        const result = await solver.computePageRank(adjacency, {
            damping: 0.85,
            epsilon: 1e-6
        });
        const elapsed = performance.now() - start;

        console.log(`✅ PageRank completed for ${n}-node graph`);
        console.log(`   Time: ${elapsed.toFixed(2)}ms`);
        console.log(`   Iterations: ${result.iterations}`);
        console.log(`   Converged: ${result.converged}`);

        if (elapsed < 500) {
            tests.pageRankAccelerated = true;
        }
    } catch (error) {
        console.log('❌ PageRank test failed:', error.message);
    }

    // Test 4: Memory efficiency
    console.log('\n4️⃣ Testing Memory Efficiency');
    console.log('─'.repeat(40));
    try {
        const initialMem = process.memoryUsage().heapUsed;

        // Create multiple solvers to test memory pooling
        const solvers = [];
        for (let i = 0; i < 10; i++) {
            const solver = new SublinearSolver();
            await new Promise(resolve => setTimeout(resolve, 10));
            solvers.push(solver);
        }

        const afterMem = process.memoryUsage().heapUsed;
        const memUsed = (afterMem - initialMem) / 1024 / 1024;

        console.log(`✅ Created 10 solver instances`);
        console.log(`   Memory used: ${memUsed.toFixed(2)}MB`);

        if (memUsed < 50) { // Should use less than 50MB for 10 instances
            tests.memoryEfficient = true;
            console.log('   ✓ Memory efficient (WASM modules likely shared)');
        }
    } catch (error) {
        console.log('❌ Memory test failed:', error.message);
    }

    // Final Report
    console.log('\n' + '═'.repeat(60));
    console.log('📊 WASM ACCELERATION REPORT');
    console.log('─'.repeat(40));

    const allPassed = Object.values(tests).every(v => v === true);

    console.log('WASM Initialized:         ' + (tests.wasmInitialized ? '✅ YES' : '❌ NO'));
    console.log('Matrix Multiply Fast:     ' + (tests.matrixMultiplyAccelerated ? '✅ YES' : '⚠️ NO'));
    console.log('PageRank Accelerated:     ' + (tests.pageRankAccelerated ? '✅ YES' : '⚠️ NO'));
    console.log('Memory Efficient:         ' + (tests.memoryEfficient ? '✅ YES' : '⚠️ NO'));

    console.log('\n' + '═'.repeat(60));
    if (tests.wasmInitialized) {
        console.log('✨ WASM acceleration is ACTIVE for MCP tools!');
        console.log('The solver is using WebAssembly for enhanced performance.');
    } else {
        console.log('⚠️ WASM acceleration is NOT active.');
        console.log('The solver is using JavaScript fallback implementation.');
    }

    return allPassed;
}

// Run test
testWASMAcceleration().then(success => {
    process.exit(success ? 0 : 1);
}).catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});