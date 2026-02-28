/**
 * AST Parser - Tree-sitter based code parsing
 *
 * Provides real AST parsing for accurate code analysis,
 * replacing regex-based heuristics with proper parsing.
 *
 * Supports: TypeScript, JavaScript, Python, Rust, Go, Java, C/C++
 */
export declare function isTreeSitterAvailable(): boolean;
export interface ASTNode {
    type: string;
    text: string;
    startPosition: {
        row: number;
        column: number;
    };
    endPosition: {
        row: number;
        column: number;
    };
    children: ASTNode[];
    parent?: string;
}
export interface FunctionInfo {
    name: string;
    params: string[];
    returnType?: string;
    async: boolean;
    exported: boolean;
    startLine: number;
    endLine: number;
    complexity: number;
    calls: string[];
}
export interface ClassInfo {
    name: string;
    extends?: string;
    implements: string[];
    methods: FunctionInfo[];
    properties: string[];
    exported: boolean;
    startLine: number;
    endLine: number;
}
export interface ImportInfo {
    source: string;
    default?: string;
    named: string[];
    namespace?: string;
    type: 'esm' | 'commonjs' | 'dynamic';
}
export interface ExportInfo {
    name: string;
    type: 'default' | 'named' | 'all';
    source?: string;
}
export interface FileAnalysis {
    file: string;
    language: string;
    imports: ImportInfo[];
    exports: ExportInfo[];
    functions: FunctionInfo[];
    classes: ClassInfo[];
    variables: string[];
    types: string[];
    complexity: number;
    lines: number;
    parseTime: number;
}
export declare class CodeParser {
    private parser;
    private initialized;
    init(): Promise<boolean>;
    /**
     * Detect language from file extension
     */
    detectLanguage(file: string): string;
    /**
     * Parse a file and return the AST
     */
    parse(file: string, content?: string): Promise<ASTNode | null>;
    private convertNode;
    /**
     * Analyze a file for functions, classes, imports, etc.
     */
    analyze(file: string, content?: string): Promise<FileAnalysis>;
    private analyzeTree;
    private parseImport;
    private parseExport;
    private parseFunction;
    private parseClass;
    private findChild;
    private getIdentifierName;
    private calculateComplexity;
    private analyzeWithRegex;
    /**
     * Get all symbols (functions, classes, types) in a file
     */
    getSymbols(file: string): Promise<string[]>;
    /**
     * Get the call graph for a file
     */
    getCallGraph(file: string): Promise<Map<string, string[]>>;
}
export declare function getCodeParser(): CodeParser;
export declare function initCodeParser(): Promise<CodeParser>;
export default CodeParser;
//# sourceMappingURL=ast-parser.d.ts.map