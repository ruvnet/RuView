// Create a larger diagonally dominant matrix to test TRUE O(log n) algorithms
import fs from 'fs';

const n = 200; // Large enough to trigger JL dimension reduction
const values = [];
const rowIndices = [];
const colIndices = [];

// Create a tridiagonal diagonally dominant matrix
for (let i = 0; i < n; i++) {
    // Diagonal element
    values.push(4.0);
    rowIndices.push(i);
    colIndices.push(i);

    // Off-diagonal elements
    if (i > 0) {
        values.push(-1.0);
        rowIndices.push(i);
        colIndices.push(i - 1);
    }
    if (i < n - 1) {
        values.push(-1.0);
        rowIndices.push(i);
        colIndices.push(i + 1);
    }
}

const matrix = { values, rowIndices, colIndices, rows: n, cols: n };
const vector = new Array(n).fill(1.0);

console.log('Matrix size:', n);
console.log('Expected JL dimension:', Math.ceil(Math.log2(n) * 8));
console.log('Matrix entries:', values.length);
console.log('Test data created successfully');

// Export for use with MCP tools
const testData = { matrix, vector, n };
fs.writeFileSync('/tmp/large-matrix-test.json', JSON.stringify(testData, null, 2));
console.log('Test data saved to /tmp/large-matrix-test.json');