/**
 * SONA Wrapper - Self-Optimizing Neural Architecture
 *
 * Provides a safe, flexible interface to @ruvector/sona with:
 * - Automatic array type conversion (Array <-> Float64Array)
 * - Graceful handling when sona is not installed
 * - TypeScript types for all APIs
 *
 * SONA Features:
 * - Micro-LoRA: Ultra-fast rank-1/2 adaptations (~0.1ms)
 * - Base-LoRA: Deeper adaptations for complex patterns
 * - EWC++: Elastic Weight Consolidation to prevent catastrophic forgetting
 * - ReasoningBank: Pattern storage and retrieval
 * - Trajectory tracking: Record and learn from execution paths
 */
/** Array input type - accepts both regular arrays and typed arrays */
export type ArrayInput = number[] | Float32Array | Float64Array;
/** SONA configuration options */
export interface SonaConfig {
    /** Hidden dimension size (required) */
    hiddenDim: number;
    /** Embedding dimension (defaults to hiddenDim) */
    embeddingDim?: number;
    /** Micro-LoRA rank (1-2, default: 1) */
    microLoraRank?: number;
    /** Base LoRA rank (default: 8) */
    baseLoraRank?: number;
    /** Micro-LoRA learning rate (default: 0.001) */
    microLoraLr?: number;
    /** Base LoRA learning rate (default: 0.0001) */
    baseLoraLr?: number;
    /** EWC lambda regularization (default: 1000.0) */
    ewcLambda?: number;
    /** Number of pattern clusters (default: 50) */
    patternClusters?: number;
    /** Trajectory buffer capacity (default: 10000) */
    trajectoryCapacity?: number;
    /** Background learning interval in ms (default: 3600000 = 1 hour) */
    backgroundIntervalMs?: number;
    /** Quality threshold for learning (default: 0.5) */
    qualityThreshold?: number;
    /** Enable SIMD optimizations (default: true) */
    enableSimd?: boolean;
}
/** Learned pattern from ReasoningBank */
export interface LearnedPattern {
    /** Pattern identifier */
    id: string;
    /** Cluster centroid embedding */
    centroid: number[];
    /** Number of trajectories in cluster */
    clusterSize: number;
    /** Total weight of trajectories */
    totalWeight: number;
    /** Average quality of member trajectories */
    avgQuality: number;
    /** Creation timestamp */
    createdAt: string;
    /** Last access timestamp */
    lastAccessed: string;
    /** Total access count */
    accessCount: number;
    /** Pattern type */
    patternType: string;
}
/** SONA engine statistics */
export interface SonaStats {
    trajectoriesRecorded: number;
    patternsLearned: number;
    microLoraUpdates: number;
    baseLoraUpdates: number;
    ewcConsolidations: number;
    avgLearningTimeMs: number;
}
/** Check if sona is available */
export declare function isSonaAvailable(): boolean;
/**
 * SONA Engine - Self-Optimizing Neural Architecture
 *
 * Provides runtime-adaptive learning with:
 * - Micro-LoRA for instant adaptations
 * - Base-LoRA for deeper learning
 * - EWC++ for preventing forgetting
 * - ReasoningBank for pattern storage
 *
 * @example
 * ```typescript
 * import { Sona } from 'ruvector';
 *
 * // Create engine with hidden dimension
 * const engine = new Sona.Engine(256);
 *
 * // Or with custom config
 * const engine = Sona.Engine.withConfig({
 *   hiddenDim: 256,
 *   microLoraRank: 2,
 *   patternClusters: 100
 * });
 *
 * // Record a trajectory
 * const trajId = engine.beginTrajectory([0.1, 0.2, ...]);
 * engine.addStep(trajId, activations, attentionWeights, 0.8);
 * engine.endTrajectory(trajId, 0.9);
 *
 * // Apply learned adaptations
 * const adapted = engine.applyMicroLora(input);
 * ```
 */
export declare class SonaEngine {
    private _native;
    /**
     * Create a new SONA engine
     * @param hiddenDim Hidden dimension size (e.g., 256, 512, 768)
     */
    constructor(hiddenDim: number);
    /**
     * Create engine with custom configuration
     * @param config SONA configuration options
     */
    static withConfig(config: SonaConfig): SonaEngine;
    /**
     * Begin recording a new trajectory
     * @param queryEmbedding Initial query embedding
     * @returns Trajectory ID for subsequent operations
     */
    beginTrajectory(queryEmbedding: ArrayInput): number;
    /**
     * Add a step to an active trajectory
     * @param trajectoryId Trajectory ID from beginTrajectory
     * @param activations Layer activations
     * @param attentionWeights Attention weights
     * @param reward Reward signal for this step (0.0 - 1.0)
     */
    addStep(trajectoryId: number, activations: ArrayInput, attentionWeights: ArrayInput, reward: number): void;
    /**
     * Alias for addStep for API compatibility
     */
    addTrajectoryStep(trajectoryId: number, activations: ArrayInput, attentionWeights: ArrayInput, reward: number): void;
    /**
     * Set the model route for a trajectory
     * @param trajectoryId Trajectory ID
     * @param route Model route identifier (e.g., "gpt-4", "claude-3")
     */
    setRoute(trajectoryId: number, route: string): void;
    /**
     * Add context to a trajectory
     * @param trajectoryId Trajectory ID
     * @param contextId Context identifier
     */
    addContext(trajectoryId: number, contextId: string): void;
    /**
     * Complete a trajectory and submit for learning
     * @param trajectoryId Trajectory ID
     * @param quality Final quality score (0.0 - 1.0)
     */
    endTrajectory(trajectoryId: number, quality: number): void;
    /**
     * Apply micro-LoRA transformation (ultra-fast, ~0.1ms)
     * @param input Input vector
     * @returns Transformed output vector
     */
    applyMicroLora(input: ArrayInput): number[];
    /**
     * Apply base-LoRA transformation to a specific layer
     * @param layerIdx Layer index
     * @param input Input vector
     * @returns Transformed output vector
     */
    applyBaseLora(layerIdx: number, input: ArrayInput): number[];
    /**
     * Run background learning cycle if due
     * Call this periodically (e.g., every few seconds)
     * @returns Status message if learning occurred, null otherwise
     */
    tick(): string | null;
    /**
     * Force immediate background learning cycle
     * @returns Status message with learning results
     */
    forceLearn(): string;
    /**
     * Flush pending instant loop updates
     */
    flush(): void;
    /**
     * Find similar learned patterns to a query
     * @param queryEmbedding Query embedding
     * @param k Number of patterns to return
     * @returns Array of similar patterns
     */
    findPatterns(queryEmbedding: ArrayInput, k: number): LearnedPattern[];
    /**
     * Get engine statistics
     * @returns Statistics object
     */
    getStats(): SonaStats;
    /**
     * Enable or disable the engine
     * @param enabled Whether to enable
     */
    setEnabled(enabled: boolean): void;
    /**
     * Check if engine is enabled
     */
    isEnabled(): boolean;
}
/**
 * SONA namespace with all exports
 */
export declare const Sona: {
    Engine: typeof SonaEngine;
    isAvailable: typeof isSonaAvailable;
};
export default Sona;
//# sourceMappingURL=sona-wrapper.d.ts.map