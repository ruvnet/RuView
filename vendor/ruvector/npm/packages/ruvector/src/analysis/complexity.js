"use strict";
/**
 * Complexity Analysis Module - Consolidated code complexity metrics
 *
 * Single source of truth for cyclomatic complexity and code metrics.
 * Used by native-worker.ts and parallel-workers.ts
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.DEFAULT_THRESHOLDS = void 0;
exports.analyzeFile = analyzeFile;
exports.analyzeFiles = analyzeFiles;
exports.exceedsThresholds = exceedsThresholds;
exports.getComplexityRating = getComplexityRating;
exports.filterComplex = filterComplex;
const fs = __importStar(require("fs"));
exports.DEFAULT_THRESHOLDS = {
    complexity: 10,
    functions: 30,
    lines: 500,
    avgSize: 50,
};
/**
 * Analyze complexity of a single file
 */
function analyzeFile(filePath, content) {
    try {
        const fileContent = content ?? (fs.existsSync(filePath) ? fs.readFileSync(filePath, 'utf-8') : '');
        if (!fileContent) {
            return { file: filePath, lines: 0, nonEmptyLines: 0, cyclomaticComplexity: 1, functions: 0, avgFunctionSize: 0 };
        }
        const lines = fileContent.split('\n');
        const nonEmptyLines = lines.filter(l => l.trim().length > 0).length;
        // Count branching statements for cyclomatic complexity
        const branches = (fileContent.match(/\bif\b/g)?.length || 0) +
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
    }
    catch {
        return { file: filePath, lines: 0, nonEmptyLines: 0, cyclomaticComplexity: 1, functions: 0, avgFunctionSize: 0 };
    }
}
/**
 * Analyze complexity of multiple files
 */
function analyzeFiles(files, maxFiles = 100) {
    return files.slice(0, maxFiles).map(f => analyzeFile(f));
}
/**
 * Check if complexity exceeds thresholds
 */
function exceedsThresholds(result, thresholds = exports.DEFAULT_THRESHOLDS) {
    return (result.cyclomaticComplexity > thresholds.complexity ||
        result.functions > thresholds.functions ||
        result.lines > thresholds.lines ||
        result.avgFunctionSize > thresholds.avgSize);
}
/**
 * Get complexity rating
 */
function getComplexityRating(complexity) {
    if (complexity <= 5)
        return 'low';
    if (complexity <= 10)
        return 'medium';
    if (complexity <= 20)
        return 'high';
    return 'critical';
}
/**
 * Filter files exceeding thresholds
 */
function filterComplex(results, thresholds = exports.DEFAULT_THRESHOLDS) {
    return results.filter(r => exceedsThresholds(r, thresholds));
}
exports.default = {
    DEFAULT_THRESHOLDS: exports.DEFAULT_THRESHOLDS,
    analyzeFile,
    analyzeFiles,
    exceedsThresholds,
    getComplexityRating,
    filterComplex,
};
//# sourceMappingURL=complexity.js.map