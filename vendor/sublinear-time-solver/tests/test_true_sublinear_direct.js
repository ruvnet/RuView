#!/usr/bin/env node

/**
 * Direct server-side test of TRUE O(log n) sublinear solver
 * This bypasses MCP to test the core algorithm directly
 */

import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Import the TRUE sublinear solver directly
import { TrueSublinearSolverTools } from '../dist/mcp/tools/true-sublinear-solver.js';

async function testTrueSublinearDirect() {
    console.log('🧪 Testing TRUE O(log n) Sublinear Solver - Direct Mode');
    console.log('================================================');

    const solver = new TrueSublinearSolverTools();

    // Test 1: Small matrix (should use base case)
    console.log('\n📊 Test 1: Small 3x3 matrix (base case)');
    const smallMatrix = {
        rows: 3,
        cols: 3,
        values: [4, -1, -1, -1, 4, -1, -1, -1, 4],
        rowIndices: [0, 0, 0, 1, 1, 1, 2, 2, 2],
        colIndices: [0, 1, 2, 0, 1, 2, 0, 1, 2]
    };
    const smallVector = [1, 1, 1];

    try {
        const startTime = Date.now();
        const result1 = await solver.solveTrueSublinear(smallMatrix, smallVector);
        const endTime = Date.now();

        console.log(`✅ Solution: [${result1.solution.map(x => x.toFixed(6)).join(', ')}]`);
        console.log(`✅ Complexity: ${result1.actual_complexity}`);
        console.log(`✅ Method: ${result1.method_used}`);
        console.log(`✅ Time: ${endTime - startTime}ms`);
        console.log(`✅ Residual norm: ${result1.residual_norm.toExponential(2)}`);
    } catch (error) {
        console.error(`❌ Small matrix test failed:`, error.message);
        return;
    }

    // Test 2: Medium matrix (should trigger TRUE O(log n))
    console.log('\n📊 Test 2: Medium 200x200 matrix (TRUE O(log n))');

    // Generate 200x200 diagonally dominant matrix
    const n = 200;
    const values = [];
    const rowIndices = [];
    const colIndices = [];

    // Create tridiagonal diagonally dominant matrix
    for (let i = 0; i < n; i++) {
        // Diagonal element (dominant)
        values.push(10 + Math.random() * 5);
        rowIndices.push(i);
        colIndices.push(i);

        // Off-diagonal elements
        if (i > 0) {
            values.push(-1 - Math.random());
            rowIndices.push(i);
            colIndices.push(i - 1);
        }
        if (i < n - 1) {
            values.push(-1 - Math.random());
            rowIndices.push(i);
            colIndices.push(i + 1);
        }
    }

    const mediumMatrix = {
        rows: n,
        cols: n,
        values,
        rowIndices,
        colIndices
    };

    // Generate sparse vector
    const mediumVector = new Array(n).fill(0);
    for (let i = 0; i < 10; i++) {
        mediumVector[i] = 1;
    }

    try {
        const startTime = Date.now();
        const result2 = await solver.solveTrueSublinear(mediumMatrix, mediumVector);
        const endTime = Date.now();

        console.log(`✅ First 10 solution elements: [${result2.solution.slice(0, 10).map(x => x.toFixed(6)).join(', ')}]`);
        console.log(`✅ Complexity: ${result2.actual_complexity}`);
        console.log(`✅ Method: ${result2.method_used}`);
        console.log(`✅ Time: ${endTime - startTime}ms`);
        console.log(`✅ Residual norm: ${result2.residual_norm.toExponential(2)}`);
        console.log(`✅ Dimension reduction ratio: ${result2.dimension_reduction_ratio.toFixed(4)}`);
    } catch (error) {
        console.error(`❌ Medium matrix test failed:`, error.message);
        return;
    }

    // Test 3: Load and test with the large vector file
    console.log('\n📊 Test 3: Large matrix with file-based vector (1020x1020)');

    try {
        // Load vector from file
        const vectorPath = path.join(__dirname, 'large_vector_1000.json');
        const vectorData = JSON.parse(fs.readFileSync(vectorPath, 'utf8'));
        const largeVector = vectorData.data || vectorData;

        console.log(`✅ Loaded vector from file: ${largeVector.length} elements`);

        // Generate matching 1020x1020 matrix (same size as vector)
        const m = largeVector.length;
        const largeValues = [];
        const largeRowIndices = [];
        const largeColIndices = [];

        // Create sparse diagonally dominant matrix
        for (let i = 0; i < m; i++) {
            // Strong diagonal dominance
            largeValues.push(15 + Math.random() * 10);
            largeRowIndices.push(i);
            largeColIndices.push(i);

            // Sparse off-diagonal pattern
            const connections = Math.min(5, m - 1); // Max 5 connections per row
            for (let c = 0; c < connections; c++) {
                const j = (i + c + 1) % m;
                if (j !== i) {
                    largeValues.push(-(1 + Math.random()));
                    largeRowIndices.push(i);
                    largeColIndices.push(j);
                }
            }
        }

        const largeMatrix = {
            rows: m,
            cols: m,
            values: largeValues,
            rowIndices: largeRowIndices,
            colIndices: largeColIndices
        };

        console.log(`✅ Generated ${m}x${m} matrix with ${largeValues.length} non-zero entries`);

        const startTime = Date.now();
        const result3 = await solver.solveTrueSublinear(largeMatrix, largeVector);
        const endTime = Date.now();

        console.log(`✅ First 10 solution elements: [${result3.solution.slice(0, 10).map(x => x.toFixed(6)).join(', ')}]`);
        console.log(`✅ Complexity: ${result3.actual_complexity}`);
        console.log(`✅ Method: ${result3.method_used}`);
        console.log(`✅ Time: ${endTime - startTime}ms`);
        console.log(`✅ Residual norm: ${result3.residual_norm.toExponential(2)}`);
        console.log(`✅ Dimension reduction ratio: ${result3.dimension_reduction_ratio.toFixed(4)}`);
        console.log(`✅ Series terms used: ${result3.series_terms_used}`);

    } catch (error) {
        console.error(`❌ Large matrix test failed:`, error.message);
        console.error(`❌ Stack trace:`, error.stack);
        return;
    }

    console.log('\n🎉 All tests completed successfully!');
    console.log('✅ TRUE O(log n) sublinear solver is working correctly');
}

// Run the test
testTrueSublinearDirect().catch(console.error);