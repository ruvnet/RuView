/**
 * Complexity Analysis Module - Consolidated code complexity metrics
 *
 * Single source of truth for cyclomatic complexity and code metrics.
 * Used by native-worker.ts and parallel-workers.ts
 */
export interface ComplexityResult {
    file: string;
    lines: number;
    nonEmptyLines: number;
    cyclomaticComplexity: number;
    functions: number;
    avgFunctionSize: number;
    maxFunctionComplexity?: number;
}
export interface ComplexityThresholds {
    complexity: number;
    functions: number;
    lines: number;
    avgSize: number;
}
export declare const DEFAULT_THRESHOLDS: ComplexityThresholds;
/**
 * Analyze complexity of a single file
 */
export declare function analyzeFile(filePath: string, content?: string): ComplexityResult;
/**
 * Analyze complexity of multiple files
 */
export declare function analyzeFiles(files: string[], maxFiles?: number): ComplexityResult[];
/**
 * Check if complexity exceeds thresholds
 */
export declare function exceedsThresholds(result: ComplexityResult, thresholds?: ComplexityThresholds): boolean;
/**
 * Get complexity rating
 */
export declare function getComplexityRating(complexity: number): 'low' | 'medium' | 'high' | 'critical';
/**
 * Filter files exceeding thresholds
 */
export declare function filterComplex(results: ComplexityResult[], thresholds?: ComplexityThresholds): ComplexityResult[];
declare const _default: {
    DEFAULT_THRESHOLDS: ComplexityThresholds;
    analyzeFile: typeof analyzeFile;
    analyzeFiles: typeof analyzeFiles;
    exceedsThresholds: typeof exceedsThresholds;
    getComplexityRating: typeof getComplexityRating;
    filterComplex: typeof filterComplex;
};
export default _default;
//# sourceMappingURL=complexity.d.ts.map