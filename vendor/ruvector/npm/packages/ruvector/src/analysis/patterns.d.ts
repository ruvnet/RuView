/**
 * Pattern Extraction Module - Consolidated code pattern detection
 *
 * Single source of truth for extracting functions, imports, exports, etc.
 * Used by native-worker.ts and parallel-workers.ts
 */
export interface PatternMatch {
    type: 'function' | 'class' | 'import' | 'export' | 'todo' | 'variable' | 'type';
    match: string;
    file: string;
    line?: number;
}
export interface FilePatterns {
    file: string;
    language: string;
    functions: string[];
    classes: string[];
    imports: string[];
    exports: string[];
    todos: string[];
    variables: string[];
}
/**
 * Detect language from file extension
 */
export declare function detectLanguage(file: string): string;
/**
 * Extract function names from content
 */
export declare function extractFunctions(content: string): string[];
/**
 * Extract class names from content
 */
export declare function extractClasses(content: string): string[];
/**
 * Extract import statements from content
 */
export declare function extractImports(content: string): string[];
/**
 * Extract export statements from content
 */
export declare function extractExports(content: string): string[];
/**
 * Extract TODO/FIXME comments from content
 */
export declare function extractTodos(content: string): string[];
/**
 * Extract all patterns from a file
 */
export declare function extractAllPatterns(filePath: string, content?: string): FilePatterns;
/**
 * Extract patterns from multiple files
 */
export declare function extractFromFiles(files: string[], maxFiles?: number): FilePatterns[];
/**
 * Convert FilePatterns to PatternMatch array (for native-worker compatibility)
 */
export declare function toPatternMatches(patterns: FilePatterns): PatternMatch[];
declare const _default: {
    detectLanguage: typeof detectLanguage;
    extractFunctions: typeof extractFunctions;
    extractClasses: typeof extractClasses;
    extractImports: typeof extractImports;
    extractExports: typeof extractExports;
    extractTodos: typeof extractTodos;
    extractAllPatterns: typeof extractAllPatterns;
    extractFromFiles: typeof extractFromFiles;
    toPatternMatches: typeof toPatternMatches;
};
export default _default;
//# sourceMappingURL=patterns.d.ts.map