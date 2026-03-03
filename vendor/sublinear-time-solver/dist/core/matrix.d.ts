/**
 * Core matrix operations for sublinear-time solvers
 */
import { Matrix, SparseMatrix, DenseMatrix, Vector, MatrixAnalysis } from './types.js';
export declare class MatrixOperations {
    /**
     * Validates matrix format and properties
     */
    static validateMatrix(matrix: Matrix): void;
    /**
     * Matrix-vector multiplication: result = matrix * vector
     */
    static multiplyMatrixVector(matrix: Matrix, vector: Vector): Vector;
    /**
     * Get matrix entry at (row, col)
     */
    static getEntry(matrix: Matrix, row: number, col: number): number;
    /**
     * Get diagonal entry at position i
     */
    static getDiagonal(matrix: Matrix, i: number): number;
    /**
     * Extract diagonal as vector
     */
    static getDiagonalVector(matrix: Matrix): Vector;
    /**
     * Get row sum for diagonal dominance check
     */
    static getRowSum(matrix: Matrix, row: number, excludeDiagonal?: boolean): number;
    /**
     * Get column sum for diagonal dominance check
     */
    static getColumnSum(matrix: Matrix, col: number, excludeDiagonal?: boolean): number;
    /**
     * Check if matrix is diagonally dominant
     */
    static checkDiagonalDominance(matrix: Matrix): {
        isRowDD: boolean;
        isColDD: boolean;
        strength: number;
    };
    /**
     * Check if matrix is symmetric
     */
    static isSymmetric(matrix: Matrix, tolerance?: number): boolean;
    /**
     * Calculate sparsity ratio (fraction of zero entries)
     */
    static calculateSparsity(matrix: Matrix): number;
    /**
     * Analyze matrix properties
     */
    static analyzeMatrix(matrix: Matrix): MatrixAnalysis;
    /**
     * Convert dense matrix to COO sparse format
     */
    static denseToSparse(dense: DenseMatrix, tolerance?: number): SparseMatrix;
    /**
     * Convert COO sparse matrix to dense format
     */
    static sparseToDense(sparse: SparseMatrix): DenseMatrix;
}
