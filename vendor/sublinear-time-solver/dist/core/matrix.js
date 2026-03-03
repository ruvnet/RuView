/**
 * Core matrix operations for sublinear-time solvers
 */
import { SolverError, ErrorCodes } from './types.js';
export class MatrixOperations {
    /**
     * Validates matrix format and properties
     */
    static validateMatrix(matrix) {
        if (!matrix) {
            throw new SolverError('Matrix is required', ErrorCodes.INVALID_MATRIX);
        }
        if (matrix.rows <= 0 || matrix.cols <= 0) {
            throw new SolverError('Matrix dimensions must be positive', ErrorCodes.INVALID_DIMENSIONS);
        }
        if (matrix.format === 'dense') {
            const dense = matrix;
            if (!Array.isArray(dense.data) || dense.data.length !== dense.rows) {
                throw new SolverError('Dense matrix data must be array of rows', ErrorCodes.INVALID_MATRIX);
            }
            for (let i = 0; i < dense.rows; i++) {
                if (!Array.isArray(dense.data[i]) || dense.data[i].length !== dense.cols) {
                    throw new SolverError(`Row ${i} has invalid length`, ErrorCodes.INVALID_MATRIX);
                }
            }
        }
        else if (matrix.format === 'coo') {
            const sparse = matrix;
            const { values, rowIndices, colIndices } = sparse;
            if (!Array.isArray(values) || !Array.isArray(rowIndices) || !Array.isArray(colIndices)) {
                throw new SolverError('COO matrix must have values, rowIndices, and colIndices arrays', ErrorCodes.INVALID_MATRIX);
            }
            if (values.length !== rowIndices.length || values.length !== colIndices.length) {
                throw new SolverError('COO matrix arrays must have same length', ErrorCodes.INVALID_MATRIX);
            }
            // Check indices are valid
            for (let i = 0; i < rowIndices.length; i++) {
                if (rowIndices[i] < 0 || rowIndices[i] >= sparse.rows) {
                    throw new SolverError(`Invalid row index ${rowIndices[i]}`, ErrorCodes.INVALID_MATRIX);
                }
                if (colIndices[i] < 0 || colIndices[i] >= sparse.cols) {
                    throw new SolverError(`Invalid column index ${colIndices[i]}`, ErrorCodes.INVALID_MATRIX);
                }
            }
        }
        else {
            throw new SolverError(`Unsupported matrix format: ${matrix.format}`, ErrorCodes.INVALID_MATRIX);
        }
    }
    /**
     * Matrix-vector multiplication: result = matrix * vector
     */
    static multiplyMatrixVector(matrix, vector) {
        this.validateMatrix(matrix);
        if (vector.length !== matrix.cols) {
            throw new SolverError(`Vector length ${vector.length} does not match matrix columns ${matrix.cols}`, ErrorCodes.INVALID_DIMENSIONS);
        }
        const result = new Array(matrix.rows).fill(0);
        if (matrix.format === 'dense') {
            const dense = matrix;
            for (let i = 0; i < matrix.rows; i++) {
                for (let j = 0; j < matrix.cols; j++) {
                    result[i] += dense.data[i][j] * vector[j];
                }
            }
        }
        else if (matrix.format === 'coo') {
            const sparse = matrix;
            for (let k = 0; k < sparse.values.length; k++) {
                const row = sparse.rowIndices[k];
                const col = sparse.colIndices[k];
                const val = sparse.values[k];
                result[row] += val * vector[col];
            }
        }
        return result;
    }
    /**
     * Get matrix entry at (row, col)
     */
    static getEntry(matrix, row, col) {
        this.validateMatrix(matrix);
        if (row < 0 || row >= matrix.rows || col < 0 || col >= matrix.cols) {
            throw new SolverError(`Index (${row}, ${col}) out of bounds`, ErrorCodes.INVALID_DIMENSIONS);
        }
        if (matrix.format === 'dense') {
            const dense = matrix;
            return dense.data[row][col];
        }
        else if (matrix.format === 'coo') {
            const sparse = matrix;
            for (let k = 0; k < sparse.values.length; k++) {
                if (sparse.rowIndices[k] === row && sparse.colIndices[k] === col) {
                    return sparse.values[k];
                }
            }
            return 0; // Implicit zero
        }
        return 0;
    }
    /**
     * Get diagonal entry at position i
     */
    static getDiagonal(matrix, i) {
        return this.getEntry(matrix, i, i);
    }
    /**
     * Extract diagonal as vector
     */
    static getDiagonalVector(matrix) {
        if (matrix.rows !== matrix.cols) {
            throw new SolverError('Matrix must be square to extract diagonal', ErrorCodes.INVALID_DIMENSIONS);
        }
        const diagonal = new Array(matrix.rows);
        for (let i = 0; i < matrix.rows; i++) {
            diagonal[i] = this.getDiagonal(matrix, i);
        }
        return diagonal;
    }
    /**
     * Get row sum for diagonal dominance check
     */
    static getRowSum(matrix, row, excludeDiagonal = false) {
        this.validateMatrix(matrix);
        if (row < 0 || row >= matrix.rows) {
            throw new SolverError(`Row index ${row} out of bounds`, ErrorCodes.INVALID_DIMENSIONS);
        }
        let sum = 0;
        if (matrix.format === 'dense') {
            const dense = matrix;
            for (let j = 0; j < matrix.cols; j++) {
                if (!excludeDiagonal || j !== row) {
                    sum += Math.abs(dense.data[row][j]);
                }
            }
        }
        else if (matrix.format === 'coo') {
            const sparse = matrix;
            for (let k = 0; k < sparse.values.length; k++) {
                if (sparse.rowIndices[k] === row) {
                    const col = sparse.colIndices[k];
                    if (!excludeDiagonal || col !== row) {
                        sum += Math.abs(sparse.values[k]);
                    }
                }
            }
        }
        return sum;
    }
    /**
     * Get column sum for diagonal dominance check
     */
    static getColumnSum(matrix, col, excludeDiagonal = false) {
        this.validateMatrix(matrix);
        if (col < 0 || col >= matrix.cols) {
            throw new SolverError(`Column index ${col} out of bounds`, ErrorCodes.INVALID_DIMENSIONS);
        }
        let sum = 0;
        if (matrix.format === 'dense') {
            const dense = matrix;
            for (let i = 0; i < matrix.rows; i++) {
                if (!excludeDiagonal || i !== col) {
                    sum += Math.abs(dense.data[i][col]);
                }
            }
        }
        else if (matrix.format === 'coo') {
            const sparse = matrix;
            for (let k = 0; k < sparse.values.length; k++) {
                if (sparse.colIndices[k] === col) {
                    const row = sparse.rowIndices[k];
                    if (!excludeDiagonal || row !== col) {
                        sum += Math.abs(sparse.values[k]);
                    }
                }
            }
        }
        return sum;
    }
    /**
     * Check if matrix is diagonally dominant
     */
    static checkDiagonalDominance(matrix) {
        this.validateMatrix(matrix);
        if (matrix.rows !== matrix.cols) {
            return { isRowDD: false, isColDD: false, strength: 0 };
        }
        let isRowDD = true;
        let isColDD = true;
        let minRowStrength = Infinity;
        let minColStrength = Infinity;
        for (let i = 0; i < matrix.rows; i++) {
            const diagonal = Math.abs(this.getDiagonal(matrix, i));
            const rowOffDiagonalSum = this.getRowSum(matrix, i, true);
            const colOffDiagonalSum = this.getColumnSum(matrix, i, true);
            if (diagonal === 0) {
                isRowDD = false;
                isColDD = false;
                minRowStrength = 0;
                minColStrength = 0;
                break;
            }
            const rowStrength = diagonal - rowOffDiagonalSum;
            const colStrength = diagonal - colOffDiagonalSum;
            if (rowStrength < 0) {
                isRowDD = false;
            }
            else {
                minRowStrength = Math.min(minRowStrength, rowStrength / diagonal);
            }
            if (colStrength < 0) {
                isColDD = false;
            }
            else {
                minColStrength = Math.min(minColStrength, colStrength / diagonal);
            }
        }
        const strength = Math.max(isRowDD ? minRowStrength : 0, isColDD ? minColStrength : 0);
        return { isRowDD, isColDD, strength };
    }
    /**
     * Check if matrix is symmetric
     */
    static isSymmetric(matrix, tolerance = 1e-10) {
        this.validateMatrix(matrix);
        if (matrix.rows !== matrix.cols) {
            return false;
        }
        // For sparse matrices, this is more complex - we'd need to compare all entries
        if (matrix.format === 'dense') {
            const dense = matrix;
            for (let i = 0; i < matrix.rows; i++) {
                for (let j = i + 1; j < matrix.cols; j++) {
                    if (Math.abs(dense.data[i][j] - dense.data[j][i]) > tolerance) {
                        return false;
                    }
                }
            }
            return true;
        }
        // For sparse matrices, check symmetry by comparing entries
        for (let i = 0; i < matrix.rows; i++) {
            for (let j = i + 1; j < matrix.cols; j++) {
                const entry_ij = this.getEntry(matrix, i, j);
                const entry_ji = this.getEntry(matrix, j, i);
                if (Math.abs(entry_ij - entry_ji) > tolerance) {
                    return false;
                }
            }
        }
        return true;
    }
    /**
     * Calculate sparsity ratio (fraction of zero entries)
     */
    static calculateSparsity(matrix) {
        this.validateMatrix(matrix);
        const totalEntries = matrix.rows * matrix.cols;
        if (matrix.format === 'dense') {
            const dense = matrix;
            let nonZeros = 0;
            for (let i = 0; i < matrix.rows; i++) {
                for (let j = 0; j < matrix.cols; j++) {
                    if (Math.abs(dense.data[i][j]) > 1e-15) {
                        nonZeros++;
                    }
                }
            }
            return 1 - (nonZeros / totalEntries);
        }
        else if (matrix.format === 'coo') {
            const sparse = matrix;
            return 1 - (sparse.values.length / totalEntries);
        }
        return 0;
    }
    /**
     * Analyze matrix properties
     */
    static analyzeMatrix(matrix) {
        this.validateMatrix(matrix);
        const dominance = this.checkDiagonalDominance(matrix);
        const isSymmetric = this.isSymmetric(matrix);
        const sparsity = this.calculateSparsity(matrix);
        let dominanceType = 'none';
        if (dominance.isRowDD && dominance.isColDD) {
            dominanceType = 'row'; // Prefer row if both
        }
        else if (dominance.isRowDD) {
            dominanceType = 'row';
        }
        else if (dominance.isColDD) {
            dominanceType = 'column';
        }
        return {
            isDiagonallyDominant: dominance.isRowDD || dominance.isColDD,
            dominanceType,
            dominanceStrength: dominance.strength,
            isSymmetric,
            sparsity,
            size: { rows: matrix.rows, cols: matrix.cols }
        };
    }
    /**
     * Convert dense matrix to COO sparse format
     */
    static denseToSparse(dense, tolerance = 1e-15) {
        const values = [];
        const rowIndices = [];
        const colIndices = [];
        for (let i = 0; i < dense.rows; i++) {
            for (let j = 0; j < dense.cols; j++) {
                const value = dense.data[i][j];
                if (Math.abs(value) > tolerance) {
                    values.push(value);
                    rowIndices.push(i);
                    colIndices.push(j);
                }
            }
        }
        return {
            rows: dense.rows,
            cols: dense.cols,
            values,
            rowIndices,
            colIndices,
            format: 'coo'
        };
    }
    /**
     * Convert COO sparse matrix to dense format
     */
    static sparseToDense(sparse) {
        const data = Array(sparse.rows).fill(null).map(() => Array(sparse.cols).fill(0));
        for (let k = 0; k < sparse.values.length; k++) {
            const row = sparse.rowIndices[k];
            const col = sparse.colIndices[k];
            const val = sparse.values[k];
            data[row][col] = val;
        }
        return {
            rows: sparse.rows,
            cols: sparse.cols,
            data,
            format: 'dense'
        };
    }
}
