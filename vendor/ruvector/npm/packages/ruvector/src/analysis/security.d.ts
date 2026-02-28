/**
 * Security Analysis Module - Consolidated security scanning
 *
 * Single source of truth for security patterns and vulnerability detection.
 * Used by native-worker.ts and parallel-workers.ts
 */
export interface SecurityPattern {
    pattern: RegExp;
    rule: string;
    severity: 'low' | 'medium' | 'high' | 'critical';
    message: string;
    suggestion?: string;
}
export interface SecurityFinding {
    file: string;
    line: number;
    severity: 'low' | 'medium' | 'high' | 'critical';
    rule: string;
    message: string;
    match?: string;
    suggestion?: string;
}
/**
 * Default security patterns for vulnerability detection
 */
export declare const SECURITY_PATTERNS: SecurityPattern[];
/**
 * Scan a single file for security issues
 */
export declare function scanFile(filePath: string, content?: string, patterns?: SecurityPattern[]): SecurityFinding[];
/**
 * Scan multiple files for security issues
 */
export declare function scanFiles(files: string[], patterns?: SecurityPattern[], maxFiles?: number): SecurityFinding[];
/**
 * Get severity score (for sorting/filtering)
 */
export declare function getSeverityScore(severity: string): number;
/**
 * Sort findings by severity (highest first)
 */
export declare function sortBySeverity(findings: SecurityFinding[]): SecurityFinding[];
declare const _default: {
    SECURITY_PATTERNS: SecurityPattern[];
    scanFile: typeof scanFile;
    scanFiles: typeof scanFiles;
    getSeverityScore: typeof getSeverityScore;
    sortBySeverity: typeof sortBySeverity;
};
export default _default;
//# sourceMappingURL=security.d.ts.map