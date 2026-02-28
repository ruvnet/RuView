/**
 * Native Worker Runner for RuVector
 *
 * Direct integration with:
 * - ONNX embedder (384d, SIMD-accelerated)
 * - VectorDB (HNSW indexing)
 * - Intelligence engine (Q-learning, memory)
 *
 * No delegation to external tools - pure ruvector execution.
 */
import { WorkerConfig, WorkerResult } from './types';
/**
 * Native Worker Runner
 */
export declare class NativeWorker {
    private config;
    private vectorDb;
    private findings;
    private stats;
    constructor(config: WorkerConfig);
    /**
     * Initialize worker with capabilities
     */
    init(): Promise<void>;
    /**
     * Run all phases in sequence
     */
    run(targetPath?: string): Promise<WorkerResult>;
    /**
     * Execute a single phase
     */
    private executePhase;
    /**
     * Phase: File Discovery
     */
    private phaseFileDiscovery;
    /**
     * Phase: Pattern Extraction (uses shared analysis module)
     */
    private phasePatternExtraction;
    /**
     * Phase: Embedding Generation (ONNX)
     */
    private phaseEmbeddingGeneration;
    /**
     * Phase: Vector Storage
     */
    private phaseVectorStorage;
    /**
     * Phase: Similarity Search
     */
    private phaseSimilaritySearch;
    /**
     * Phase: Security Scan (uses shared analysis module)
     */
    private phaseSecurityScan;
    /**
     * Phase: Complexity Analysis (uses shared analysis module)
     */
    private phaseComplexityAnalysis;
    /**
     * Phase: Summarization
     */
    private phaseSummarization;
    /**
     * Summarize phase data for results
     */
    private summarizePhaseData;
}
/**
 * Quick worker factory functions
 */
export declare function createSecurityWorker(name?: string): NativeWorker;
export declare function createAnalysisWorker(name?: string): NativeWorker;
export declare function createLearningWorker(name?: string): NativeWorker;
//# sourceMappingURL=native-worker.d.ts.map