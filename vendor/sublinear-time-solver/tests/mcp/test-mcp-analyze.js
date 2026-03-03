#!/usr/bin/env node

/**
 * Test MCP analyzeMatrix with different formats
 */

import { mcp__sublinear-solver__analyzeMatrix } from '@modelcontextprotocol/server-sublinear-solver';

async function testAnalyzeMatrix() {
  console.log('Testing MCP analyzeMatrix functionality\n');

  // Test 1: Small dense matrix (should work)
  console.log('Test 1: Small 5x5 dense matrix');
  try {
    const smallMatrix = {
      rows: 5,
      cols: 5,
      format: 'dense',
      data: [
        [10, -1, -0.5, 0, 0],
        [-1, 10, -1, -0.5, 0],
        [-0.5, -1, 10, -1, -0.5],
        [0, -0.5, -1, 10, -1],
        [0, 0, -0.5, -1, 10]
      ]
    };

    const result = await mcp__sublinear-solver__analyzeMatrix({
      matrix: smallMatrix,
      checkDominance: true,
      checkSymmetry: true
    });
    console.log('✅ Small matrix analysis succeeded');
    console.log('   Diagonally dominant:', result.isDiagonallyDominant);
    console.log('   Symmetric:', result.isSymmetric);
    console.log('   Sparsity:', (result.sparsity * 100).toFixed(1) + '%');
  } catch (error) {
    console.log('❌ Error:', error.message);
  }

  // Test 2: Large dense matrix (this is probably where it fails)
  console.log('\nTest 2: Large 1000x1000 dense matrix (generated)');
  try {
    // Generate a proper 1000x1000 matrix
    const size = 1000;
    const data = [];
    for (let i = 0; i < size; i++) {
      const row = new Array(size).fill(0);
      // Diagonal element
      row[i] = 10;
      // A few off-diagonal elements for sparsity
      if (i > 0) row[i - 1] = -1;
      if (i < size - 1) row[i + 1] = -0.5;
      data.push(row);
    }

    const largeMatrix = {
      rows: size,
      cols: size,
      format: 'dense',
      data: data
    };

    // This might fail due to size limits in MCP
    const result = await mcp__sublinear-solver__analyzeMatrix({
      matrix: largeMatrix,
      checkDominance: true,
      checkSymmetry: false, // Skip symmetry check for speed
      computeGap: false,
      estimateCondition: false
    });
    console.log('✅ Large matrix analysis succeeded');
    console.log('   Diagonally dominant:', result.isDiagonallyDominant);
    console.log('   Sparsity:', (result.sparsity * 100).toFixed(1) + '%');
  } catch (error) {
    console.log('❌ Error:', error.message);
    console.log('   This is likely due to MCP size limits');
  }

  // Test 3: Use sparse format instead (recommended for large matrices)
  console.log('\nTest 3: Large 1000x1000 sparse matrix (COO format)');
  try {
    const size = 1000;
    const values = [];
    const rowIndices = [];
    const colIndices = [];

    // Generate tridiagonal matrix in sparse format
    for (let i = 0; i < size; i++) {
      // Diagonal
      values.push(10);
      rowIndices.push(i);
      colIndices.push(i);

      // Lower diagonal
      if (i > 0) {
        values.push(-1);
        rowIndices.push(i);
        colIndices.push(i - 1);
      }

      // Upper diagonal
      if (i < size - 1) {
        values.push(-0.5);
        rowIndices.push(i);
        colIndices.push(i + 1);
      }
    }

    const sparseMatrix = {
      rows: size,
      cols: size,
      format: 'coo',
      values: values,
      rowIndices: rowIndices,
      colIndices: colIndices
    };

    const result = await mcp__sublinear-solver__analyzeMatrix({
      matrix: sparseMatrix,
      checkDominance: true,
      checkSymmetry: false,
      computeGap: false,
      estimateCondition: false
    });
    console.log('✅ Sparse matrix analysis succeeded');
    console.log('   Diagonally dominant:', result.isDiagonallyDominant);
    console.log('   Sparsity:', (result.sparsity * 100).toFixed(1) + '%');
    console.log('   Non-zero elements:', values.length);
    console.log('   Memory efficiency:', ((values.length / (size * size)) * 100).toFixed(2) + '%');
  } catch (error) {
    console.log('❌ Error:', error.message);
  }

  // Recommendation
  console.log('\n📊 Recommendation:');
  console.log('For large matrices (>100x100), use sparse COO format instead of dense format.');
  console.log('This avoids MCP serialization limits and is much more memory efficient.');
  console.log('\nExample conversion:');
  console.log(`
// Instead of dense format:
matrix = {
  format: 'dense',
  rows: 1000, cols: 1000,
  data: [[...], [...], ...] // 1M elements!
}

// Use sparse COO format:
matrix = {
  format: 'coo',
  rows: 1000, cols: 1000,
  values: [10, -1, ...],      // Only non-zeros
  rowIndices: [0, 0, ...],    // Row for each value
  colIndices: [0, 1, ...]     // Column for each value
}
`);
}

// Check if this is a direct MCP call or a test script
const isMCP = typeof mcp__sublinear-solver__analyzeMatrix === 'function';

if (!isMCP) {
  console.log('This script needs to be run through MCP.');
  console.log('The issue you\'re seeing is likely because:');
  console.log('1. The dense matrix is being truncated during MCP serialization');
  console.log('2. Only the first 5 rows are being sent instead of all 1000 rows');
  console.log('\nSolution: Use sparse (COO) format for large matrices!');
} else {
  testAnalyzeMatrix().catch(console.error);
}