/**
 * RuVector WASM Unified - Learning Engine
 *
 * Provides adaptive learning mechanisms including:
 * - Micro-LoRA adaptation for efficient fine-tuning
 * - SONA pre-query processing for enhanced embeddings
 * - BTSP one-shot learning for rapid pattern acquisition
 * - Reinforcement learning integration
 * - Continual learning support
 */
import type { EnhancedEmbedding, LearningTrajectory, MicroLoraConfig, BtspConfig, QueryDag, LearningConfig } from './types';
/**
 * Core learning engine for adaptive model updates and pattern learning
 */
export interface LearningEngine {
    /**
     * Micro-LoRA adaptation for operation-specific fine-tuning
     * Applies low-rank updates based on operation type
     * @param embedding Input embedding to adapt
     * @param opType Operation type identifier (e.g., 'attention', 'ffn', 'norm')
     * @param config Optional LoRA configuration
     * @returns Adapted embedding with low-rank modifications
     */
    microLoraAdapt(embedding: Float32Array, opType: string, config?: Partial<MicroLoraConfig>): Float32Array;
    /**
     * SONA (Self-Organizing Neural Architecture) pre-query processing
     * Enhances embeddings before attention computation
     * @param dag Query DAG with embeddings
     * @param contextWindow Context window size
     * @returns Enhanced embedding with context
     */
    sonaPreQuery(dag: QueryDag, contextWindow?: number): EnhancedEmbedding;
    /**
     * BTSP (Behavioral Timescale Synaptic Plasticity) one-shot learning
     * Rapidly acquires new patterns with single exposure
     * @param pattern Pattern to learn
     * @param signal Reward/error signal for reinforcement
     * @param config Optional BTSP configuration
     */
    btspOneShotLearn(pattern: Float32Array, signal: number, config?: Partial<BtspConfig>): void;
    /**
     * Update policy from trajectory
     * @param trajectory Learning trajectory with states, actions, rewards
     * @param algorithm RL algorithm to use
     * @returns Policy gradient and metrics
     */
    updateFromTrajectory(trajectory: LearningTrajectory, algorithm?: RLAlgorithm): PolicyUpdate;
    /**
     * Compute advantage estimates for policy gradient
     * @param rewards Reward sequence
     * @param values Value estimates
     * @param gamma Discount factor
     * @param lambda GAE lambda parameter
     * @returns Advantage estimates
     */
    computeAdvantages(rewards: Float32Array, values: Float32Array, gamma?: number, lambda?: number): Float32Array;
    /**
     * Get action from current policy
     * @param state Current state embedding
     * @param temperature Sampling temperature
     * @returns Action index and log probability
     */
    sampleAction(state: Float32Array, temperature?: number): {
        action: number;
        logProb: number;
    };
    /**
     * Elastic weight consolidation for preventing catastrophic forgetting
     * @param taskId Current task identifier
     * @param importance Fisher information matrix diagonal
     */
    ewcRegularize(taskId: string, importance?: Float32Array): void;
    /**
     * Progressive neural networks - add new column for task
     * @param taskId New task identifier
     * @param hiddenSize Size of hidden layers in new column
     */
    progressiveAddColumn(taskId: string, hiddenSize?: number): void;
    /**
     * Experience replay for continual learning
     * @param bufferSize Maximum replay buffer size
     * @param batchSize Batch size for replay
     * @returns Replayed batch
     */
    experienceReplay(bufferSize?: number, batchSize?: number): ReplayBatch;
    /**
     * MAML-style meta-learning inner loop
     * @param supportSet Support set for few-shot learning
     * @param innerSteps Number of inner loop steps
     * @param innerLr Inner loop learning rate
     * @returns Adapted parameters
     */
    mamlInnerLoop(supportSet: TaskDataset, innerSteps?: number, innerLr?: number): Float32Array;
    /**
     * Reptile meta-learning update
     * @param taskBatch Batch of tasks for meta-learning
     * @param epsilon Interpolation factor
     */
    reptileUpdate(taskBatch: TaskDataset[], epsilon?: number): void;
    /**
     * Get current learning statistics
     */
    getStats(): LearningStats;
    /**
     * Reset learning state
     * @param keepWeights Whether to keep learned weights
     */
    reset(keepWeights?: boolean): void;
    /**
     * Save learning checkpoint
     * @param path Checkpoint path
     */
    saveCheckpoint(path: string): Promise<void>;
    /**
     * Load learning checkpoint
     * @param path Checkpoint path
     */
    loadCheckpoint(path: string): Promise<void>;
}
/** Reinforcement learning algorithm */
export type RLAlgorithm = 'ppo' | 'a2c' | 'dqn' | 'sac' | 'td3' | 'reinforce';
/** Policy update result */
export interface PolicyUpdate {
    gradient: Float32Array;
    loss: number;
    entropy: number;
    klDivergence: number;
    clipFraction?: number;
}
/** Experience replay batch */
export interface ReplayBatch {
    states: Float32Array[];
    actions: number[];
    rewards: number[];
    nextStates: Float32Array[];
    dones: boolean[];
    priorities?: Float32Array;
}
/** Task dataset for meta-learning */
export interface TaskDataset {
    taskId: string;
    supportInputs: Float32Array[];
    supportLabels: number[];
    queryInputs: Float32Array[];
    queryLabels: number[];
}
/** Learning statistics */
export interface LearningStats {
    totalSteps: number;
    totalEpisodes: number;
    averageReward: number;
    averageLoss: number;
    learningRate: number;
    memoryUsage: number;
    patternsLearned: number;
    adaptationCount: number;
}
/** LoRA layer info */
export interface LoraLayerInfo {
    layerName: string;
    rank: number;
    alpha: number;
    enabled: boolean;
    parameterCount: number;
}
/**
 * Create a learning engine instance
 * @param config Optional configuration
 * @returns Initialized learning engine
 */
export declare function createLearningEngine(config?: LearningConfig): LearningEngine;
/**
 * Create Micro-LoRA configuration
 * @param rank LoRA rank (default: 8)
 * @param alpha LoRA alpha scaling (default: 16)
 * @param targetModules Modules to apply LoRA to
 */
export declare function createMicroLoraConfig(rank?: number, alpha?: number, targetModules?: string[]): MicroLoraConfig;
/**
 * Create BTSP configuration for one-shot learning
 * @param learningRate Learning rate for plasticity
 * @param eligibilityDecay Decay rate for eligibility traces
 * @param rewardWindow Time window for reward integration
 */
export declare function createBtspConfig(learningRate?: number, eligibilityDecay?: number, rewardWindow?: number): BtspConfig;
/**
 * Compute cosine annealing learning rate
 * @param step Current step
 * @param totalSteps Total training steps
 * @param lrMax Maximum learning rate
 * @param lrMin Minimum learning rate
 */
export declare function cosineAnnealingLr(step: number, totalSteps: number, lrMax?: number, lrMin?: number): number;
/**
 * Compute warmup learning rate
 * @param step Current step
 * @param warmupSteps Number of warmup steps
 * @param targetLr Target learning rate after warmup
 */
export declare function warmupLr(step: number, warmupSteps: number, targetLr?: number): number;
//# sourceMappingURL=learning.d.ts.map