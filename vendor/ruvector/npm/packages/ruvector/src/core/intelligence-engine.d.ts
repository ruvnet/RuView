/**
 * IntelligenceEngine - Full RuVector Intelligence Stack
 *
 * Integrates all RuVector capabilities for self-learning hooks:
 * - VectorDB with HNSW for semantic memory (150x faster)
 * - SONA for continual learning (Micro-LoRA, EWC++)
 * - FastAgentDB for episode/trajectory storage
 * - Attention mechanisms for pattern recognition
 * - ReasoningBank for pattern clustering
 *
 * Replaces the simple Q-learning approach with real ML-powered intelligence.
 */
import { EpisodeSearchResult } from './agentdb-fast';
import { SonaConfig, LearnedPattern } from './sona-wrapper';
import { ParallelConfig, BatchEpisode } from './parallel-intelligence';
export interface MemoryEntry {
    id: string;
    content: string;
    type: string;
    embedding: number[];
    created: string;
    accessed: number;
    score?: number;
}
export interface AgentRoute {
    agent: string;
    confidence: number;
    reason: string;
    patterns?: LearnedPattern[];
    alternates?: Array<{
        agent: string;
        confidence: number;
    }>;
}
export interface LearningStats {
    totalMemories: number;
    memoryDimensions: number;
    totalEpisodes: number;
    totalTrajectories: number;
    avgReward: number;
    sonaEnabled: boolean;
    trajectoriesRecorded: number;
    patternsLearned: number;
    microLoraUpdates: number;
    baseLoraUpdates: number;
    ewcConsolidations: number;
    routingPatterns: number;
    errorPatterns: number;
    coEditPatterns: number;
    workerTriggers: number;
    attentionEnabled: boolean;
    onnxEnabled: boolean;
    parallelEnabled: boolean;
    parallelWorkers: number;
    parallelBusy: number;
    parallelQueued: number;
}
export interface IntelligenceConfig {
    /** Embedding dimension for vectors (default: 256, 384 for ONNX) */
    embeddingDim?: number;
    /** Maximum memories to store (default: 100000) */
    maxMemories?: number;
    /** Maximum episodes for trajectory storage (default: 50000) */
    maxEpisodes?: number;
    /** Enable SONA continual learning (default: true if available) */
    enableSona?: boolean;
    /** Enable attention mechanisms (default: true if available) */
    enableAttention?: boolean;
    /** Enable ONNX semantic embeddings (default: false, opt-in for quality) */
    enableOnnx?: boolean;
    /** SONA configuration */
    sonaConfig?: Partial<SonaConfig>;
    /** Storage path for persistence */
    storagePath?: string;
    /** Learning rate for pattern updates (default: 0.1) */
    learningRate?: number;
    /**
     * Enable parallel workers for batch operations
     * Auto-enabled for MCP servers, disabled for CLI hooks
     */
    parallelConfig?: Partial<ParallelConfig>;
}
/**
 * Full-stack intelligence engine using all RuVector capabilities
 */
export declare class IntelligenceEngine {
    private config;
    private vectorDb;
    private agentDb;
    private sona;
    private attention;
    private onnxEmbedder;
    private onnxReady;
    private parallel;
    private memories;
    private routingPatterns;
    private errorPatterns;
    private coEditPatterns;
    private agentMappings;
    private workerTriggerMappings;
    private currentTrajectoryId;
    private sessionStart;
    private learningEnabled;
    private episodeBatchQueue;
    constructor(config?: IntelligenceConfig);
    private initOnnx;
    private initVectorDb;
    private initParallel;
    /**
     * Generate embedding using ONNX, attention, or hash (in order of preference)
     */
    embed(text: string): number[];
    /**
     * Async embedding with ONNX support (recommended for semantic quality)
     */
    embedAsync(text: string): Promise<number[]>;
    /**
     * Attention-based embedding using Flash or Multi-head attention
     */
    private attentionEmbed;
    /**
     * Improved hash-based embedding with positional encoding
     */
    private hashEmbed;
    private tokenize;
    private tokenEmbed;
    private meanPool;
    /**
     * Store content in vector memory (uses ONNX if available)
     */
    remember(content: string, type?: string): Promise<MemoryEntry>;
    /**
     * Semantic search of memories (uses ONNX if available)
     */
    recall(query: string, topK?: number): Promise<MemoryEntry[]>;
    private cosineSimilarity;
    /**
     * Route a task to the best agent using learned patterns
     */
    route(task: string, file?: string): Promise<AgentRoute>;
    private getExtension;
    private getState;
    private getAlternates;
    /**
     * Begin recording a trajectory (before edit/command)
     */
    beginTrajectory(context: string, file?: string): void;
    /**
     * Add a step to the current trajectory
     */
    addTrajectoryStep(activations: number[], reward: number): void;
    /**
     * End the current trajectory with a quality score
     */
    endTrajectory(success: boolean, quality?: number): void;
    /**
     * Set the agent route for current trajectory
     */
    setTrajectoryRoute(agent: string): void;
    /**
     * Record an episode for learning
     */
    recordEpisode(state: string, action: string, reward: number, nextState: string, done: boolean, metadata?: Record<string, any>): Promise<void>;
    /**
     * Queue episode for batch processing (3-4x faster with workers)
     */
    queueEpisode(episode: BatchEpisode): void;
    /**
     * Process queued episodes in parallel batch
     */
    flushEpisodeBatch(): Promise<number>;
    /**
     * Learn from similar past episodes
     */
    learnFromSimilar(state: string, k?: number): Promise<EpisodeSearchResult[]>;
    /**
     * Register worker trigger to agent mappings
     */
    registerWorkerTrigger(trigger: string, priority: string, agents: string[]): void;
    /**
     * Get agents for a worker trigger
     */
    getAgentsForTrigger(trigger: string): {
        priority: string;
        agents: string[];
    } | undefined;
    /**
     * Route a task using worker trigger patterns first, then fall back to regular routing
     */
    routeWithWorkers(task: string, file?: string): Promise<AgentRoute>;
    /**
     * Initialize default worker trigger mappings
     */
    initDefaultWorkerMappings(): void;
    /**
     * Record a co-edit pattern
     */
    recordCoEdit(file1: string, file2: string): void;
    /**
     * Get likely next files to edit
     */
    getLikelyNextFiles(file: string, topK?: number): Array<{
        file: string;
        count: number;
    }>;
    /**
     * Record an error pattern with fixes
     */
    recordErrorFix(errorPattern: string, fix: string): void;
    /**
     * Get suggested fixes for an error
     */
    getSuggestedFixes(error: string): string[];
    /**
     * Run background learning cycle
     */
    tick(): string | null;
    /**
     * Force immediate learning
     */
    forceLearn(): string | null;
    /**
     * Get comprehensive learning statistics
     */
    getStats(): LearningStats;
    /**
     * Export all data for persistence
     */
    export(): Record<string, any>;
    /**
     * Import data from persistence
     */
    import(data: Record<string, any>, merge?: boolean): void;
    /**
     * Clear all data
     */
    clear(): void;
    /** Legacy: patterns object */
    get patterns(): Record<string, Record<string, number>>;
    /** Legacy: file_sequences array */
    get file_sequences(): string[][];
    /** Legacy: errors object */
    get errors(): Record<string, string[]>;
}
/**
 * Create a new IntelligenceEngine with default settings
 */
export declare function createIntelligenceEngine(config?: IntelligenceConfig): IntelligenceEngine;
/**
 * Create a high-performance engine with all features enabled
 */
export declare function createHighPerformanceEngine(): IntelligenceEngine;
/**
 * Create a lightweight engine for fast startup
 */
export declare function createLightweightEngine(): IntelligenceEngine;
export default IntelligenceEngine;
//# sourceMappingURL=intelligence-engine.d.ts.map