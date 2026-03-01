/**
 * Parallel Intelligence - Worker-based acceleration for IntelligenceEngine
 *
 * Provides parallel processing for:
 * - Q-learning batch updates (3-4x faster)
 * - Multi-file pattern matching
 * - Background memory indexing
 * - Parallel similarity search
 * - Multi-file code analysis
 * - Parallel git commit analysis
 *
 * Uses worker_threads for CPU-bound operations, keeping hooks non-blocking.
 */
export interface ParallelConfig {
    /** Number of worker threads (default: CPU cores - 1) */
    numWorkers?: number;
    /** Enable parallel processing (default: true for MCP, false for CLI) */
    enabled?: boolean;
    /** Minimum batch size to use parallel (default: 4) */
    batchThreshold?: number;
}
export interface BatchEpisode {
    state: string;
    action: string;
    reward: number;
    nextState: string;
    done: boolean;
    metadata?: Record<string, any>;
}
export interface PatternMatchResult {
    file: string;
    patterns: Array<{
        pattern: string;
        confidence: number;
    }>;
}
export interface CoEditAnalysis {
    file1: string;
    file2: string;
    commits: string[];
    strength: number;
}
export declare class ParallelIntelligence {
    private workers;
    private taskQueue;
    private busyWorkers;
    private config;
    private initialized;
    constructor(config?: ParallelConfig);
    /**
     * Initialize worker pool
     */
    init(): Promise<void>;
    private processQueue;
    /**
     * Execute task in worker pool
     */
    private executeInWorker;
    /**
     * Batch Q-learning episode recording (3-4x faster)
     */
    recordEpisodesBatch(episodes: BatchEpisode[]): Promise<void>;
    /**
     * Multi-file pattern matching (parallel pretrain)
     */
    matchPatternsParallel(files: string[]): Promise<PatternMatchResult[]>;
    /**
     * Background memory indexing (non-blocking)
     */
    indexMemoriesBackground(memories: Array<{
        content: string;
        type: string;
    }>): Promise<void>;
    /**
     * Parallel similarity search with sharding
     */
    searchParallel(query: string, topK?: number): Promise<Array<{
        content: string;
        score: number;
    }>>;
    /**
     * Multi-file AST analysis for routing
     */
    analyzeFilesParallel(files: string[]): Promise<Map<string, {
        agent: string;
        confidence: number;
    }>>;
    /**
     * Parallel git commit analysis for co-edit detection
     */
    analyzeCommitsParallel(commits: string[]): Promise<CoEditAnalysis[]>;
    /**
     * Get worker pool stats
     */
    getStats(): {
        enabled: boolean;
        workers: number;
        busy: number;
        queued: number;
    };
    /**
     * Shutdown worker pool
     */
    shutdown(): Promise<void>;
}
export declare function getParallelIntelligence(config?: ParallelConfig): ParallelIntelligence;
export declare function initParallelIntelligence(config?: ParallelConfig): Promise<ParallelIntelligence>;
export default ParallelIntelligence;
//# sourceMappingURL=parallel-intelligence.d.ts.map