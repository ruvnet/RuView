/**
 * Parallel Workers - Extended worker capabilities for RuVector hooks
 *
 * Provides parallel processing for advanced operations:
 *
 * 1. SPECULATIVE PRE-COMPUTATION
 *    - Pre-embed likely next files based on co-edit patterns
 *    - Warm model cache before operations
 *    - Predictive route caching
 *
 * 2. REAL-TIME CODE ANALYSIS
 *    - Multi-file AST parsing with tree-sitter
 *    - Cross-file type inference
 *    - Live complexity metrics
 *    - Dependency graph updates
 *
 * 3. ADVANCED LEARNING
 *    - Distributed trajectory replay
 *    - Parallel SONA micro-LoRA updates
 *    - Background EWC consolidation
 *    - Online pattern clustering
 *
 * 4. INTELLIGENT RETRIEVAL
 *    - Parallel RAG chunking and retrieval
 *    - Sharded similarity search
 *    - Context relevance ranking
 *    - Semantic deduplication
 *
 * 5. SECURITY & QUALITY
 *    - Parallel SAST scanning
 *    - Multi-rule linting
 *    - Vulnerability detection
 *    - Code smell analysis
 *
 * 6. GIT INTELLIGENCE
 *    - Parallel blame analysis
 *    - Branch comparison
 *    - Merge conflict prediction
 *    - Code churn metrics
 */
import { SecurityFinding } from '../analysis/security';
export interface WorkerPoolConfig {
    numWorkers?: number;
    enabled?: boolean;
    taskTimeout?: number;
    maxQueueSize?: number;
}
export interface SpeculativeEmbedding {
    file: string;
    embedding: number[];
    confidence: number;
    timestamp: number;
}
export interface ASTAnalysis {
    file: string;
    language: string;
    complexity: number;
    functions: string[];
    imports: string[];
    exports: string[];
    dependencies: string[];
}
export type { SecurityFinding };
export interface ContextChunk {
    content: string;
    source: string;
    relevance: number;
    embedding?: number[];
}
export interface GitBlame {
    file: string;
    lines: Array<{
        line: number;
        author: string;
        date: string;
        commit: string;
    }>;
}
export interface CodeChurn {
    file: string;
    additions: number;
    deletions: number;
    commits: number;
    authors: string[];
    lastModified: string;
}
export declare class ExtendedWorkerPool {
    private workers;
    private taskQueue;
    private busyWorkers;
    private config;
    private initialized;
    private speculativeCache;
    private astCache;
    constructor(config?: WorkerPoolConfig);
    init(): Promise<void>;
    private getWorkerCode;
    private getWorkerHandlers;
    private handleWorkerResult;
    private processQueue;
    private execute;
    /**
     * Pre-embed files likely to be edited next based on co-edit patterns
     * Hook: session-start, post-edit
     */
    speculativeEmbed(currentFile: string, coEditGraph: Map<string, string[]>): Promise<SpeculativeEmbedding[]>;
    /**
     * Analyze AST of multiple files in parallel
     * Hook: pre-edit, route
     */
    analyzeAST(files: string[]): Promise<ASTAnalysis[]>;
    /**
     * Analyze code complexity for multiple files
     * Hook: post-edit, session-end
     */
    analyzeComplexity(files: string[]): Promise<Array<{
        file: string;
        lines: number;
        nonEmptyLines: number;
        cyclomaticComplexity: number;
        functions: number;
        avgFunctionSize: number;
    }>>;
    /**
     * Build dependency graph from entry points
     * Hook: session-start
     */
    buildDependencyGraph(entryPoints: string[]): Promise<Record<string, string[]>>;
    /**
     * Scan files for security vulnerabilities
     * Hook: pre-command (before commit), post-edit
     */
    securityScan(files: string[], rules?: string[]): Promise<SecurityFinding[]>;
    /**
     * Retrieve relevant context chunks in parallel
     * Hook: suggest-context, recall
     */
    ragRetrieve(query: string, chunks: ContextChunk[], topK?: number): Promise<ContextChunk[]>;
    /**
     * Rank context items by relevance to query
     * Hook: suggest-context
     */
    rankContext(context: string[], query: string): Promise<Array<{
        index: number;
        content: string;
        relevance: number;
    }>>;
    /**
     * Deduplicate similar items
     * Hook: remember, suggest-context
     */
    deduplicate(items: string[], threshold?: number): Promise<string[]>;
    /**
     * Get blame information for files in parallel
     * Hook: pre-edit (for context), coedit
     */
    gitBlame(files: string[]): Promise<GitBlame[]>;
    /**
     * Analyze code churn for files
     * Hook: session-start, route
     */
    gitChurn(files: string[], since?: string): Promise<CodeChurn[]>;
    getStats(): {
        enabled: boolean;
        workers: number;
        busy: number;
        queued: number;
        speculativeCacheSize: number;
        astCacheSize: number;
    };
    clearCaches(): void;
    shutdown(): Promise<void>;
}
export declare function getExtendedWorkerPool(config?: WorkerPoolConfig): ExtendedWorkerPool;
export declare function initExtendedWorkerPool(config?: WorkerPoolConfig): Promise<ExtendedWorkerPool>;
export default ExtendedWorkerPool;
//# sourceMappingURL=parallel-workers.d.ts.map