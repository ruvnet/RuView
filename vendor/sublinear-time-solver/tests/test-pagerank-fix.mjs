#!/usr/bin/env node
/**
 * Test that pageRankVector.map error has been fixed
 */

import { SublinearSolver } from './dist/core/solver.js';

console.log('🔍 TESTING PAGERANK FIX');
console.log('═'.repeat(60));

async function testPageRankFix() {
    // Test the exact same parameters that were causing the error
    const adjacency = {
        rows: 4,
        cols: 4,
        format: 'dense',
        data: [
            [0, 1, 1, 0],
            [1, 0, 1, 1],
            [1, 1, 0, 1],
            [0, 1, 1, 0]
        ]
    };

    const damping = 0.85;

    console.log('Testing PageRank with:');
    console.log('  Adjacency matrix: 4x4');
    console.log('  Damping factor:', damping);
    console.log('');

    try {
        // Create solver without config (tests default constructor)
        const solver = new SublinearSolver();

        // Call computePageRank with the same params as the error
        const result = await solver.computePageRank(adjacency, { damping });

        console.log('✅ SUCCESS: PageRank executed without error!');
        console.log('');
        console.log('Results:');
        console.log('  Ranks:', result.ranks.map(r => r.toFixed(4)));
        console.log('  Iterations:', result.iterations);
        console.log('  Converged:', result.converged);
        console.log('  Residual:', result.residual.toExponential(3));

        // Validate result structure
        if (!Array.isArray(result.ranks)) {
            throw new Error('ranks should be an array');
        }
        if (result.ranks.length !== 4) {
            throw new Error(`ranks should have 4 elements, got ${result.ranks.length}`);
        }
        if (result.ranks.some(r => typeof r !== 'number')) {
            throw new Error('all ranks should be numbers');
        }

        console.log('\n✅ Result structure is valid');
        console.log('\n' + '═'.repeat(60));
        console.log('✨ The "pageRankVector.map is not a function" error has been FIXED!');
        return true;

    } catch (error) {
        console.error('❌ FAILED:', error.message);
        console.error('Stack:', error.stack);
        console.log('\n' + '═'.repeat(60));
        console.log('⚠️ The error has NOT been fixed');
        return false;
    }
}

// Run test
testPageRankFix().then(success => {
    process.exit(success ? 0 : 1);
}).catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});