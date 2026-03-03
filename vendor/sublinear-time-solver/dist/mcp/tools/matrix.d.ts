/**
 * MCP Tools for matrix analysis and operations
 */
import { Matrix, AnalyzeMatrixParams, MatrixAnalysis } from '../../core/types.js';
export declare class MatrixTools {
    /**
     * Analyze matrix properties
     */
    static analyzeMatrix(params: AnalyzeMatrixParams): MatrixAnalysis & {
        recommendations: string[];
        performance: {
            expectedComplexity: string;
            memoryUsage: string;
            recommendedMethod: string;
        };
        visualMetrics: {
            bandwidth: number;
            profileMetric: number;
            fillRatio: number;
        };
    };
    /**
     * Check matrix conditioning and stability
     */
    static checkConditioning(matrix: Matrix): {
        isWellConditioned: boolean;
        conditionEstimate?: number;
        stabilityRating: 'excellent' | 'good' | 'fair' | 'poor';
        warnings: string[];
    };
    /**
     * Convert between matrix formats
     */
    static convertFormat(matrix: Matrix, targetFormat: 'dense' | 'coo'): Matrix;
    /**
     * Generate test matrices for benchmarking
     */
    static generateTestMatrix(type: string, size: number, params?: any): Matrix;
    private static computeBandwidth;
    private static computeProfile;
    private static predictComplexity;
    private static estimateMemoryUsage;
    private static recommendSolverMethod;
    private static generateDetailedRecommendations;
    private static estimateConditionNumber;
    private static generateDiagonallyDominantMatrix;
    private static generateLaplacianMatrix;
    private static generateRandomSparseMatrix;
    private static generateTridiagonalMatrix;
}
