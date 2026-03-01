"use strict";
/**
 * Security Analysis Module - Consolidated security scanning
 *
 * Single source of truth for security patterns and vulnerability detection.
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
exports.SECURITY_PATTERNS = void 0;
exports.scanFile = scanFile;
exports.scanFiles = scanFiles;
exports.getSeverityScore = getSeverityScore;
exports.sortBySeverity = sortBySeverity;
const fs = __importStar(require("fs"));
/**
 * Default security patterns for vulnerability detection
 */
exports.SECURITY_PATTERNS = [
    // Critical: Hardcoded secrets
    { pattern: /password\s*=\s*['"][^'"]+['"]/gi, rule: 'no-hardcoded-password', severity: 'critical', message: 'Hardcoded password detected', suggestion: 'Use environment variables or secret management' },
    { pattern: /api[_-]?key\s*=\s*['"][^'"]+['"]/gi, rule: 'no-hardcoded-apikey', severity: 'critical', message: 'Hardcoded API key detected', suggestion: 'Use environment variables' },
    { pattern: /secret\s*=\s*['"][^'"]+['"]/gi, rule: 'no-hardcoded-secret', severity: 'critical', message: 'Hardcoded secret detected', suggestion: 'Use environment variables or secret management' },
    { pattern: /private[_-]?key\s*=\s*['"][^'"]+['"]/gi, rule: 'no-hardcoded-private-key', severity: 'critical', message: 'Hardcoded private key detected', suggestion: 'Use secure key management' },
    // High: Code execution risks
    { pattern: /eval\s*\(/g, rule: 'no-eval', severity: 'high', message: 'Avoid eval() - code injection risk', suggestion: 'Use safer alternatives like JSON.parse()' },
    { pattern: /exec\s*\(/g, rule: 'no-exec', severity: 'high', message: 'Avoid exec() - command injection risk', suggestion: 'Use execFile or spawn with args array' },
    { pattern: /Function\s*\(/g, rule: 'no-function-constructor', severity: 'high', message: 'Avoid Function constructor - code injection risk' },
    { pattern: /child_process.*exec\(/g, rule: 'no-shell-exec', severity: 'high', message: 'Shell execution detected', suggestion: 'Use execFile or spawn instead' },
    // High: SQL injection
    { pattern: /SELECT\s+.*\s+FROM.*\+/gi, rule: 'sql-injection-risk', severity: 'high', message: 'Potential SQL injection - string concatenation in query', suggestion: 'Use parameterized queries' },
    { pattern: /`SELECT.*\$\{/gi, rule: 'sql-injection-template', severity: 'high', message: 'Template literal in SQL query', suggestion: 'Use parameterized queries' },
    // Medium: XSS risks
    { pattern: /dangerouslySetInnerHTML/g, rule: 'xss-risk', severity: 'medium', message: 'XSS risk: dangerouslySetInnerHTML', suggestion: 'Sanitize content before rendering' },
    { pattern: /innerHTML\s*=/g, rule: 'no-inner-html', severity: 'medium', message: 'Avoid innerHTML - XSS risk', suggestion: 'Use textContent or sanitize content' },
    { pattern: /document\.write\s*\(/g, rule: 'no-document-write', severity: 'medium', message: 'Avoid document.write - XSS risk' },
    // Medium: Other risks
    { pattern: /\$\{.*\}/g, rule: 'template-injection', severity: 'low', message: 'Template literal detected - verify no injection' },
    { pattern: /new\s+RegExp\s*\([^)]*\+/g, rule: 'regex-injection', severity: 'medium', message: 'Dynamic RegExp - potential ReDoS risk', suggestion: 'Validate/sanitize regex input' },
    { pattern: /\.on\s*\(\s*['"]error['"]/g, rule: 'unhandled-error', severity: 'low', message: 'Error handler detected - verify proper error handling' },
];
/**
 * Scan a single file for security issues
 */
function scanFile(filePath, content, patterns = exports.SECURITY_PATTERNS) {
    const findings = [];
    try {
        const fileContent = content ?? (fs.existsSync(filePath) ? fs.readFileSync(filePath, 'utf-8') : '');
        if (!fileContent)
            return findings;
        for (const { pattern, rule, severity, message, suggestion } of patterns) {
            const regex = new RegExp(pattern.source, pattern.flags);
            let match;
            while ((match = regex.exec(fileContent)) !== null) {
                const lineNum = fileContent.slice(0, match.index).split('\n').length;
                findings.push({
                    file: filePath,
                    line: lineNum,
                    severity,
                    rule,
                    message,
                    match: match[0].slice(0, 50),
                    suggestion,
                });
            }
        }
    }
    catch {
        // Skip unreadable files
    }
    return findings;
}
/**
 * Scan multiple files for security issues
 */
function scanFiles(files, patterns = exports.SECURITY_PATTERNS, maxFiles = 100) {
    const findings = [];
    for (const file of files.slice(0, maxFiles)) {
        findings.push(...scanFile(file, undefined, patterns));
    }
    return findings;
}
/**
 * Get severity score (for sorting/filtering)
 */
function getSeverityScore(severity) {
    switch (severity) {
        case 'critical': return 4;
        case 'high': return 3;
        case 'medium': return 2;
        case 'low': return 1;
        default: return 0;
    }
}
/**
 * Sort findings by severity (highest first)
 */
function sortBySeverity(findings) {
    return [...findings].sort((a, b) => getSeverityScore(b.severity) - getSeverityScore(a.severity));
}
exports.default = {
    SECURITY_PATTERNS: exports.SECURITY_PATTERNS,
    scanFile,
    scanFiles,
    getSeverityScore,
    sortBySeverity,
};
//# sourceMappingURL=security.js.map