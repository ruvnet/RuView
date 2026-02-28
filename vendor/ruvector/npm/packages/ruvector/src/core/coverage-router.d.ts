/**
 * Coverage Router - Test coverage-aware agent routing
 *
 * Uses test coverage data to make smarter routing decisions:
 * - Prioritize testing for uncovered code
 * - Route to tester agent for low-coverage files
 * - Suggest test files for modified code
 */
export interface CoverageData {
    file: string;
    lines: {
        total: number;
        covered: number;
        percentage: number;
    };
    functions: {
        total: number;
        covered: number;
        percentage: number;
    };
    branches: {
        total: number;
        covered: number;
        percentage: number;
    };
    uncoveredLines: number[];
    uncoveredFunctions: string[];
}
export interface CoverageSummary {
    files: Map<string, CoverageData>;
    overall: {
        lines: number;
        functions: number;
        branches: number;
    };
    lowCoverageFiles: string[];
    uncoveredFiles: string[];
}
export interface TestSuggestion {
    file: string;
    testFile: string;
    reason: string;
    priority: 'high' | 'medium' | 'low';
    coverage: number;
    uncoveredFunctions: string[];
}
/**
 * Parse Istanbul/NYC JSON coverage report
 */
export declare function parseIstanbulCoverage(coveragePath: string): CoverageSummary;
/**
 * Find coverage report in project
 */
export declare function findCoverageReport(projectRoot?: string): string | null;
/**
 * Get coverage data for a specific file
 */
export declare function getFileCoverage(file: string, summary?: CoverageSummary): CoverageData | null;
/**
 * Suggest tests for files based on coverage
 */
export declare function suggestTests(files: string[], summary?: CoverageSummary): TestSuggestion[];
/**
 * Determine if a file needs the tester agent based on coverage
 */
export declare function shouldRouteToTester(file: string, summary?: CoverageSummary): {
    route: boolean;
    reason: string;
    coverage: number;
};
/**
 * Get coverage-aware routing weight for agent selection
 */
export declare function getCoverageRoutingWeight(file: string, summary?: CoverageSummary): {
    coder: number;
    tester: number;
    reviewer: number;
};
declare const _default: {
    parseIstanbulCoverage: typeof parseIstanbulCoverage;
    findCoverageReport: typeof findCoverageReport;
    getFileCoverage: typeof getFileCoverage;
    suggestTests: typeof suggestTests;
    shouldRouteToTester: typeof shouldRouteToTester;
    getCoverageRoutingWeight: typeof getCoverageRoutingWeight;
};
export default _default;
//# sourceMappingURL=coverage-router.d.ts.map