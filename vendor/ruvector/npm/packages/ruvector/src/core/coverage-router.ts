/**
 * Coverage Router - Test coverage-aware agent routing
 *
 * Uses test coverage data to make smarter routing decisions:
 * - Prioritize testing for uncovered code
 * - Route to tester agent for low-coverage files
 * - Suggest test files for modified code
 */

import * as fs from 'fs';
import * as path from 'path';

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
export function parseIstanbulCoverage(coveragePath: string): CoverageSummary {
  const files = new Map<string, CoverageData>();
  const lowCoverageFiles: string[] = [];
  const uncoveredFiles: string[] = [];
  let totalLines = 0, coveredLines = 0;
  let totalFunctions = 0, coveredFunctions = 0;
  let totalBranches = 0, coveredBranches = 0;

  try {
    const coverage = JSON.parse(fs.readFileSync(coveragePath, 'utf8'));

    for (const [file, data] of Object.entries(coverage) as [string, any][]) {
      // Skip test files
      if (file.includes('.test.') || file.includes('.spec.') || file.includes('__tests__')) {
        continue;
      }

      // Parse statement coverage
      const statements = Object.values(data.s || {}) as number[];
      const linesCovered = statements.filter(n => n > 0).length;
      const linesTotal = statements.length;

      // Parse function coverage
      const functions = Object.values(data.f || {}) as number[];
      const fnCovered = functions.filter(n => n > 0).length;
      const fnTotal = functions.length;

      // Parse branch coverage
      const branches = Object.values(data.b || {}).flat() as number[];
      const brCovered = branches.filter(n => n > 0).length;
      const brTotal = branches.length;

      // Find uncovered lines
      const uncoveredLines: number[] = [];
      for (const [line, count] of Object.entries(data.s || {})) {
        if (count === 0) {
          uncoveredLines.push(parseInt(line));
        }
      }

      // Find uncovered functions
      const uncoveredFunctions: string[] = [];
      const fnMap = data.fnMap || {};
      for (const [fnId, count] of Object.entries(data.f || {})) {
        if (count === 0 && fnMap[fnId]) {
          uncoveredFunctions.push(fnMap[fnId].name || `function_${fnId}`);
        }
      }

      const linePercentage = linesTotal > 0 ? (linesCovered / linesTotal) * 100 : 100;
      const fnPercentage = fnTotal > 0 ? (fnCovered / fnTotal) * 100 : 100;
      const brPercentage = brTotal > 0 ? (brCovered / brTotal) * 100 : 100;

      files.set(file, {
        file,
        lines: { total: linesTotal, covered: linesCovered, percentage: linePercentage },
        functions: { total: fnTotal, covered: fnCovered, percentage: fnPercentage },
        branches: { total: brTotal, covered: brCovered, percentage: brPercentage },
        uncoveredLines,
        uncoveredFunctions,
      });

      totalLines += linesTotal;
      coveredLines += linesCovered;
      totalFunctions += fnTotal;
      coveredFunctions += fnCovered;
      totalBranches += brTotal;
      coveredBranches += brCovered;

      if (linePercentage < 50) {
        lowCoverageFiles.push(file);
      }
      if (linePercentage === 0 && linesTotal > 0) {
        uncoveredFiles.push(file);
      }
    }
  } catch (e) {
    // Return empty summary on error
  }

  return {
    files,
    overall: {
      lines: totalLines > 0 ? (coveredLines / totalLines) * 100 : 0,
      functions: totalFunctions > 0 ? (coveredFunctions / totalFunctions) * 100 : 0,
      branches: totalBranches > 0 ? (coveredBranches / totalBranches) * 100 : 0,
    },
    lowCoverageFiles,
    uncoveredFiles,
  };
}

/**
 * Find coverage report in project
 */
export function findCoverageReport(projectRoot: string = process.cwd()): string | null {
  const possiblePaths = [
    'coverage/coverage-final.json',
    'coverage/coverage-summary.json',
    '.nyc_output/coverage.json',
    'coverage.json',
    'coverage/lcov.info',
  ];

  for (const p of possiblePaths) {
    const fullPath = path.join(projectRoot, p);
    if (fs.existsSync(fullPath)) {
      return fullPath;
    }
  }

  return null;
}

/**
 * Get coverage data for a specific file
 */
export function getFileCoverage(file: string, summary?: CoverageSummary): CoverageData | null {
  if (!summary) {
    const reportPath = findCoverageReport();
    if (!reportPath) return null;
    summary = parseIstanbulCoverage(reportPath);
  }

  // Try exact match first
  if (summary.files.has(file)) {
    return summary.files.get(file)!;
  }

  // Try matching by basename
  const basename = path.basename(file);
  for (const [key, data] of summary.files) {
    if (key.endsWith(file) || key.endsWith(basename)) {
      return data;
    }
  }

  return null;
}

/**
 * Suggest tests for files based on coverage
 */
export function suggestTests(files: string[], summary?: CoverageSummary): TestSuggestion[] {
  if (!summary) {
    const reportPath = findCoverageReport();
    if (reportPath) {
      summary = parseIstanbulCoverage(reportPath);
    }
  }

  const suggestions: TestSuggestion[] = [];

  for (const file of files) {
    const coverage = summary ? getFileCoverage(file, summary) : null;

    // Determine test file path
    const ext = path.extname(file);
    const base = path.basename(file, ext);
    const dir = path.dirname(file);

    const possibleTestFiles = [
      path.join(dir, `${base}.test${ext}`),
      path.join(dir, `${base}.spec${ext}`),
      path.join(dir, '__tests__', `${base}.test${ext}`),
      path.join('test', `${base}.test${ext}`),
      path.join('tests', `${base}.test${ext}`),
    ];

    const existingTestFile = possibleTestFiles.find(t => fs.existsSync(t));
    const testFile = existingTestFile || possibleTestFiles[0];

    if (!coverage) {
      suggestions.push({
        file,
        testFile,
        reason: 'No coverage data - needs test file',
        priority: 'high',
        coverage: 0,
        uncoveredFunctions: [],
      });
    } else if (coverage.lines.percentage < 30) {
      suggestions.push({
        file,
        testFile,
        reason: `Very low coverage (${coverage.lines.percentage.toFixed(1)}%)`,
        priority: 'high',
        coverage: coverage.lines.percentage,
        uncoveredFunctions: coverage.uncoveredFunctions,
      });
    } else if (coverage.lines.percentage < 70) {
      suggestions.push({
        file,
        testFile,
        reason: `Low coverage (${coverage.lines.percentage.toFixed(1)}%)`,
        priority: 'medium',
        coverage: coverage.lines.percentage,
        uncoveredFunctions: coverage.uncoveredFunctions,
      });
    } else if (coverage.uncoveredFunctions.length > 0) {
      suggestions.push({
        file,
        testFile,
        reason: `${coverage.uncoveredFunctions.length} untested functions`,
        priority: 'low',
        coverage: coverage.lines.percentage,
        uncoveredFunctions: coverage.uncoveredFunctions,
      });
    }
  }

  return suggestions.sort((a, b) => {
    const priorityOrder = { high: 0, medium: 1, low: 2 };
    return priorityOrder[a.priority] - priorityOrder[b.priority];
  });
}

/**
 * Determine if a file needs the tester agent based on coverage
 */
export function shouldRouteToTester(file: string, summary?: CoverageSummary): {
  route: boolean;
  reason: string;
  coverage: number;
} {
  const coverage = getFileCoverage(file, summary);

  if (!coverage) {
    return {
      route: true,
      reason: 'No test coverage data available',
      coverage: 0,
    };
  }

  if (coverage.lines.percentage < 50) {
    return {
      route: true,
      reason: `Low coverage: ${coverage.lines.percentage.toFixed(1)}%`,
      coverage: coverage.lines.percentage,
    };
  }

  if (coverage.uncoveredFunctions.length > 3) {
    return {
      route: true,
      reason: `${coverage.uncoveredFunctions.length} untested functions`,
      coverage: coverage.lines.percentage,
    };
  }

  return {
    route: false,
    reason: `Adequate coverage: ${coverage.lines.percentage.toFixed(1)}%`,
    coverage: coverage.lines.percentage,
  };
}

/**
 * Get coverage-aware routing weight for agent selection
 */
export function getCoverageRoutingWeight(file: string, summary?: CoverageSummary): {
  coder: number;
  tester: number;
  reviewer: number;
} {
  const coverage = getFileCoverage(file, summary);

  if (!coverage) {
    // No coverage = prioritize testing
    return { coder: 0.3, tester: 0.5, reviewer: 0.2 };
  }

  const pct = coverage.lines.percentage;

  if (pct < 30) {
    // Very low - strongly prioritize testing
    return { coder: 0.2, tester: 0.6, reviewer: 0.2 };
  } else if (pct < 60) {
    // Low - moderate testing priority
    return { coder: 0.4, tester: 0.4, reviewer: 0.2 };
  } else if (pct < 80) {
    // Okay - balanced
    return { coder: 0.5, tester: 0.3, reviewer: 0.2 };
  } else {
    // Good - focus on code quality
    return { coder: 0.5, tester: 0.2, reviewer: 0.3 };
  }
}

export default {
  parseIstanbulCoverage,
  findCoverageReport,
  getFileCoverage,
  suggestTests,
  shouldRouteToTester,
  getCoverageRoutingWeight,
};
