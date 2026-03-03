#!/usr/bin/env node
/**
 * Debug WASM execution to see why it's falling back
 */

import { SublinearSolver } from './dist/core/solver.js';

console.log('🔍 DEBUGGING WASM EXECUTION');
console.log('═'.repeat(50));

async function debugWasmExecution() {
    const solver = new SublinearSolver({
        method: 'neumann',
        epsilon: 1e-6,
        maxIterations: 100
    });

    // Wait for initialization
    await new Promise(resolve => setTimeout(resolve, 300));

    console.log('WASM Status:', solver.wasmAccelerated);
    console.log('Rust Solver Available:', !!solver.wasmModules.rustSolver);

    if (solver.wasmModules.rustSolver) {
        console.log('\n🧪 Testing direct WASM call...');

        const matrix = {
            rows: 3,
            cols: 3,
            format: 'dense',
            data: [[4, -1, 0], [-1, 4, -1], [0, -1, 4]]
        };
        const vector = [3, 2, 3];

        try {
            console.log('Calling WASM solve directly...');
            const directResult = await solver.wasmModules.rustSolver.solve(matrix, vector, 'neumann');
            console.log('✅ Direct WASM call succeeded!');
            console.log('Result:', directResult);
        } catch (error) {
            console.log('❌ Direct WASM call failed:', error.message);
            console.log('Error stack:', error.stack);
        }

        console.log('\n🔄 Testing through solver.solve()...');
        try {
            const result = await solver.solve(matrix, vector);
            console.log('Result method:', result.method);
            console.log('WASM was used:', result.method.includes('WASM'));
        } catch (error) {
            console.log('❌ Solver.solve() failed:', error.message);
        }
    } else {
        console.log('❌ No Rust solver available');
    }
}

debugWasmExecution().catch(console.error);