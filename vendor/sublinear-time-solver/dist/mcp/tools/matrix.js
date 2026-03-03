/**
 * MCP Tools for matrix analysis and operations
 */
import { MatrixOperations } from '../../core/matrix.js';
import { SolverError, ErrorCodes } from '../../core/types.js';
export class MatrixTools {
    /**
     * Analyze matrix properties
     */
    static analyzeMatrix(params) {
        MatrixOperations.validateMatrix(params.matrix);
        const analysis = MatrixOperations.analyzeMatrix(params.matrix);
        const matrix = params.matrix;
        // Enhanced analysis
        const bandwidth = this.computeBandwidth(matrix);
        const profileMetric = this.computeProfile(matrix);
        const fillRatio = 1 - analysis.sparsity;
        // Generate performance predictions
        const expectedComplexity = this.predictComplexity(analysis, matrix);
        const memoryUsage = this.estimateMemoryUsage(matrix);
        const recommendedMethod = this.recommendSolverMethod(analysis);
        // Generate recommendations
        const recommendations = this.generateDetailedRecommendations(analysis, {
            bandwidth,
            profileMetric,
            fillRatio,
            size: matrix.rows
        });
        return {
            ...analysis,
            recommendations,
            performance: {
                expectedComplexity,
                memoryUsage,
                recommendedMethod
            },
            visualMetrics: {
                bandwidth,
                profileMetric,
                fillRatio
            }
        };
    }
    /**
     * Check matrix conditioning and stability
     */
    static checkConditioning(matrix) {
        const analysis = MatrixOperations.analyzeMatrix(matrix);
        const warnings = [];
        // Check diagonal dominance strength
        let stabilityRating = 'excellent';
        if (!analysis.isDiagonallyDominant) {
            warnings.push('Matrix is not diagonally dominant');
            stabilityRating = 'poor';
        }
        else if (analysis.dominanceStrength < 0.1) {
            warnings.push('Weak diagonal dominance - may converge slowly');
            stabilityRating = 'fair';
        }
        else if (analysis.dominanceStrength < 0.5) {
            stabilityRating = 'good';
        }
        // Check for zero or near-zero diagonals
        const diagonals = MatrixOperations.getDiagonalVector(matrix);
        const nearZeroDiagonals = diagonals.filter(d => Math.abs(d) < 1e-12);
        if (nearZeroDiagonals.length > 0) {
            warnings.push(`${nearZeroDiagonals.length} near-zero diagonal elements detected`);
            stabilityRating = 'poor';
        }
        // Rough condition number estimate for small matrices
        let conditionEstimate;
        if (matrix.rows <= 100 && matrix.format === 'dense') {
            conditionEstimate = this.estimateConditionNumber(matrix);
            if (conditionEstimate > 1e12) {
                warnings.push('Very high condition number - matrix is nearly singular');
                stabilityRating = 'poor';
            }
            else if (conditionEstimate > 1e6) {
                warnings.push('High condition number - may have numerical issues');
                if (stabilityRating === 'excellent')
                    stabilityRating = 'fair';
            }
        }
        return {
            isWellConditioned: warnings.length === 0 && analysis.isDiagonallyDominant,
            conditionEstimate,
            stabilityRating,
            warnings
        };
    }
    /**
     * Convert between matrix formats
     */
    static convertFormat(matrix, targetFormat) {
        MatrixOperations.validateMatrix(matrix);
        if (matrix.format === targetFormat) {
            return matrix;
        }
        if (targetFormat === 'dense') {
            return MatrixOperations.sparseToDense(matrix);
        }
        else {
            return MatrixOperations.denseToSparse(matrix);
        }
    }
    /**
     * Generate test matrices for benchmarking
     */
    static generateTestMatrix(type, size, params = {}) {
        switch (type) {
            case 'diagonally-dominant':
                return this.generateDiagonallyDominantMatrix(size, params.strength || 2.0);
            case 'laplacian':
                return this.generateLaplacianMatrix(size, params.connectivity || 0.1);
            case 'random-sparse':
                return this.generateRandomSparseMatrix(size, params.density || 0.1, params.dominance || true);
            case 'tridiagonal':
                return this.generateTridiagonalMatrix(size, params.offDiagonal || -1);
            default:
                throw new SolverError(`Unknown test matrix type: ${type}`, ErrorCodes.INVALID_PARAMETERS);
        }
    }
    static computeBandwidth(matrix) {
        if (matrix.format === 'dense') {
            let maxBandwidth = 0;
            for (let i = 0; i < matrix.rows; i++) {
                for (let j = 0; j < matrix.cols; j++) {
                    if (Math.abs(MatrixOperations.getEntry(matrix, i, j)) > 1e-15) {
                        maxBandwidth = Math.max(maxBandwidth, Math.abs(i - j));
                    }
                }
            }
            return maxBandwidth;
        }
        else {
            const sparse = matrix;
            let maxBandwidth = 0;
            for (let k = 0; k < sparse.values.length; k++) {
                const bandwidth = Math.abs(sparse.rowIndices[k] - sparse.colIndices[k]);
                maxBandwidth = Math.max(maxBandwidth, bandwidth);
            }
            return maxBandwidth;
        }
    }
    static computeProfile(matrix) {
        let profile = 0;
        for (let i = 0; i < matrix.rows; i++) {
            let firstNonZero = matrix.cols;
            for (let j = 0; j <= i; j++) {
                if (Math.abs(MatrixOperations.getEntry(matrix, i, j)) > 1e-15) {
                    firstNonZero = j;
                    break;
                }
            }
            profile += (i - firstNonZero + 1);
        }
        return profile;
    }
    static predictComplexity(analysis, matrix) {
        const n = matrix.rows;
        const nnz = Math.round((1 - analysis.sparsity) * n * n);
        if (analysis.isDiagonallyDominant) {
            if (analysis.dominanceStrength > 0.5) {
                return `O(nnz * log n) ≈ O(${nnz} * ${Math.ceil(Math.log2(n))})`;
            }
            else {
                return `O(nnz * n^0.5) ≈ O(${nnz} * ${Math.ceil(Math.sqrt(n))})`;
            }
        }
        else {
            return `O(n^3) ≈ O(${n}^3) - not suitable for sublinear methods`;
        }
    }
    static estimateMemoryUsage(matrix) {
        const n = matrix.rows;
        const elementSize = 8; // 64-bit floats
        if (matrix.format === 'dense') {
            const mb = (n * n * elementSize) / (1024 * 1024);
            return `${mb.toFixed(1)} MB (dense)`;
        }
        else {
            const sparse = matrix;
            const mb = (sparse.values.length * 3 * elementSize) / (1024 * 1024); // values + 2 index arrays
            return `${mb.toFixed(1)} MB (sparse)`;
        }
    }
    static recommendSolverMethod(analysis) {
        if (!analysis.isDiagonallyDominant) {
            return 'Direct solver (LU/Cholesky) - matrix not suitable for sublinear methods';
        }
        if (analysis.isSymmetric) {
            return 'Neumann series or Forward Push (symmetric case)';
        }
        else {
            if (analysis.dominanceStrength > 0.3) {
                return 'Random Walk or Bidirectional Push';
            }
            else {
                return 'Forward Push with preconditioning';
            }
        }
    }
    static generateDetailedRecommendations(analysis, metrics) {
        const recommendations = [];
        if (!analysis.isDiagonallyDominant) {
            recommendations.push('Matrix is not diagonally dominant. Consider matrix preconditioning or regularization.');
            recommendations.push('Use direct solvers (LU, QR) instead of iterative methods.');
        }
        else {
            if (analysis.dominanceStrength < 0.1) {
                recommendations.push('Weak diagonal dominance. Consider diagonal scaling or row equilibration.');
            }
            if (analysis.sparsity > 0.95) {
                recommendations.push('Extremely sparse matrix. Use sparse storage formats and specialized algorithms.');
            }
            if (metrics.bandwidth > analysis.size.rows * 0.1) {
                recommendations.push('Large bandwidth detected. Consider matrix reordering (RCM, AMD).');
            }
            if (metrics.size > 10000) {
                recommendations.push('Large matrix. Consider sublinear estimation for specific entries rather than full solve.');
                recommendations.push('Use random walk sampling for single coordinate queries.');
            }
            if (!analysis.isSymmetric) {
                recommendations.push('Asymmetric matrix. Random walk methods may be most effective.');
                recommendations.push('Consider bidirectional push for better convergence.');
            }
        }
        if (metrics.fillRatio > 0.5) {
            recommendations.push('Dense matrix. Memory usage may be significant for large sizes.');
        }
        return recommendations;
    }
    static estimateConditionNumber(matrix) {
        // Very rough estimate using diagonal dominance
        if (matrix.format !== 'dense' || matrix.rows > 100) {
            return NaN;
        }
        const diagonals = MatrixOperations.getDiagonalVector(matrix);
        const maxDiag = Math.max(...diagonals.map(Math.abs));
        const minDiag = Math.min(...diagonals.map(Math.abs));
        if (minDiag === 0) {
            return Infinity;
        }
        return maxDiag / minDiag; // Very rough approximation
    }
    static generateDiagonallyDominantMatrix(size, strength) {
        const data = Array(size).fill(null).map(() => Array(size).fill(0));
        for (let i = 0; i < size; i++) {
            let offDiagSum = 0;
            // Fill off-diagonal entries
            for (let j = 0; j < size; j++) {
                if (i !== j && Math.random() < 0.3) { // 30% sparsity
                    const value = (Math.random() - 0.5) * 2;
                    data[i][j] = value;
                    offDiagSum += Math.abs(value);
                }
            }
            // Set diagonal to ensure dominance
            data[i][i] = strength * offDiagSum + 1;
        }
        return {
            rows: size,
            cols: size,
            data,
            format: 'dense'
        };
    }
    static generateLaplacianMatrix(size, connectivity) {
        const data = Array(size).fill(null).map(() => Array(size).fill(0));
        for (let i = 0; i < size; i++) {
            let degree = 0;
            for (let j = 0; j < size; j++) {
                if (i !== j && Math.random() < connectivity) {
                    data[i][j] = -1;
                    degree++;
                }
            }
            data[i][i] = degree;
        }
        return {
            rows: size,
            cols: size,
            data,
            format: 'dense'
        };
    }
    static generateRandomSparseMatrix(size, density, ensureDominance) {
        const values = [];
        const rowIndices = [];
        const colIndices = [];
        const rowSums = new Array(size).fill(0);
        // Generate off-diagonal entries
        for (let i = 0; i < size; i++) {
            for (let j = 0; j < size; j++) {
                if (i !== j && Math.random() < density) {
                    const value = (Math.random() - 0.5) * 2;
                    values.push(value);
                    rowIndices.push(i);
                    colIndices.push(j);
                    rowSums[i] += Math.abs(value);
                }
            }
        }
        // Add diagonal entries
        for (let i = 0; i < size; i++) {
            const diagValue = ensureDominance ? rowSums[i] * 1.5 + 1 : Math.random() * 5 + 1;
            values.push(diagValue);
            rowIndices.push(i);
            colIndices.push(i);
        }
        return {
            rows: size,
            cols: size,
            values,
            rowIndices,
            colIndices,
            format: 'coo'
        };
    }
    static generateTridiagonalMatrix(size, offDiagonal) {
        const values = [];
        const rowIndices = [];
        const colIndices = [];
        for (let i = 0; i < size; i++) {
            // Diagonal
            values.push(2);
            rowIndices.push(i);
            colIndices.push(i);
            // Off-diagonal
            if (i > 0) {
                values.push(offDiagonal);
                rowIndices.push(i);
                colIndices.push(i - 1);
            }
            if (i < size - 1) {
                values.push(offDiagonal);
                rowIndices.push(i);
                colIndices.push(i + 1);
            }
        }
        return {
            rows: size,
            cols: size,
            values,
            rowIndices,
            colIndices,
            format: 'coo'
        };
    }
}
