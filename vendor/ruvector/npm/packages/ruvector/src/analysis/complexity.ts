/**
 * Complexity Analysis Module - Consolidated code complexity metrics
 *
 * Single source of truth for cyclomatic complexity and code metrics.
 * Used by native-worker.ts and parallel-workers.ts
 */

import * as fs from 'fs';

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
  complexity: number;  // Max cyclomatic complexity
  functions: number;   // Max functions per file
  lines: number;       // Max lines per file
  avgSize: number;     // Max avg function size
}

export const DEFAULT_THRESHOLDS: ComplexityThresholds = {
  complexity: 10,
  functions: 30,
  lines: 500,
  avgSize: 50,
};

/**
 * Analyze complexity of a single file
 */
export function analyzeFile(filePath: string, content?: string): ComplexityResult {
  try {
    const fileContent = content ?? (fs.existsSync(filePath) ? fs.readFileSync(filePath, 'utf-8') : '');
    if (!fileContent) {
      return { file: filePath, lines: 0, nonEmptyLines: 0, cyclomaticComplexity: 1, functions: 0, avgFunctionSize: 0 };
    }

    const lines = fileContent.split('\n');
    const nonEmptyLines = lines.filter(l => l.trim().length > 0).length;

    // Count branching statements for cyclomatic complexity
    const branches =
      (fileContent.match(/\bif\b/g)?.length || 0) +
      (fileContent.match(/\belse\b/g)?.length || 0) +
      (fileContent.match(/\bfor\b/g)?.length || 0) +
      (fileContent.match(/\bwhile\b/g)?.length || 0) +
      (fileContent.match(/\bswitch\b/g)?.length || 0) +
      (fileContent.match(/\bcase\b/g)?.length || 0) +
      (fileContent.match(/\bcatch\b/g)?.length || 0) +
      (fileContent.match(/\?\?/g)?.length || 0) +
      (fileContent.match(/&&/g)?.length || 0) +
      (fileContent.match(/\|\|/g)?.length || 0) +
      (fileContent.match(/\?[^:]/g)?.length || 0); // Ternary

    const cyclomaticComplexity = branches + 1;

    // Count functions
    const functionPatterns = [
      /function\s+\w+/g,
      /\w+\s*=\s*(?:async\s*)?\(/g,
      /\w+\s*:\s*(?:async\s*)?\(/g,
      /(?:async\s+)?(?:public|private|protected)?\s+\w+\s*\([^)]*\)\s*[:{]/g,
    ];

    let functions = 0;
    for (const pattern of functionPatterns) {
      functions += (fileContent.match(pattern) || []).length;
    }
    // Deduplicate by rough estimate
    functions = Math.ceil(functions / 2);

    const avgFunctionSize = functions > 0 ? Math.round(nonEmptyLines / functions) : nonEmptyLines;

    return {
      file: filePath,
      lines: lines.length,
      nonEmptyLines,
      cyclomaticComplexity,
      functions,
      avgFunctionSize,
    };
  } catch {
    return { file: filePath, lines: 0, nonEmptyLines: 0, cyclomaticComplexity: 1, functions: 0, avgFunctionSize: 0 };
  }
}

/**
 * Analyze complexity of multiple files
 */
export function analyzeFiles(files: string[], maxFiles: number = 100): ComplexityResult[] {
  return files.slice(0, maxFiles).map(f => analyzeFile(f));
}

/**
 * Check if complexity exceeds thresholds
 */
export function exceedsThresholds(
  result: ComplexityResult,
  thresholds: ComplexityThresholds = DEFAULT_THRESHOLDS
): boolean {
  return (
    result.cyclomaticComplexity > thresholds.complexity ||
    result.functions > thresholds.functions ||
    result.lines > thresholds.lines ||
    result.avgFunctionSize > thresholds.avgSize
  );
}

/**
 * Get complexity rating
 */
export function getComplexityRating(complexity: number): 'low' | 'medium' | 'high' | 'critical' {
  if (complexity <= 5) return 'low';
  if (complexity <= 10) return 'medium';
  if (complexity <= 20) return 'high';
  return 'critical';
}

/**
 * Filter files exceeding thresholds
 */
export function filterComplex(
  results: ComplexityResult[],
  thresholds: ComplexityThresholds = DEFAULT_THRESHOLDS
): ComplexityResult[] {
  return results.filter(r => exceedsThresholds(r, thresholds));
}

export default {
  DEFAULT_THRESHOLDS,
  analyzeFile,
  analyzeFiles,
  exceedsThresholds,
  getComplexityRating,
  filterComplex,
};
