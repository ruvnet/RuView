/**
 * Training Pipeline for SONA
 *
 * Comprehensive training infrastructure with metrics tracking,
 * learning rate scheduling, and checkpoint management.
 *
 * @example
 * ```typescript
 * import { TrainingPipeline, TrainingConfig } from '@ruvector/ruvllm';
 *
 * const pipeline = new TrainingPipeline({
 *   learningRate: 0.001,
 *   batchSize: 32,
 *   epochs: 10,
 * });
 *
 * // Add training data
 * pipeline.addBatch(inputs, targets, qualities);
 *
 * // Run training
 * const result = pipeline.train();
 * console.log(`Final loss: ${result.finalLoss}`);
 * ```
 */
import { Embedding, TrainingConfig, TrainingResult } from './types';
import { LoraAdapter } from './lora';
import { EwcManager } from './sona';
/**
 * Training metrics
 */
export interface TrainingMetrics {
    /** Current epoch */
    epoch: number;
    /** Current step */
    step: number;
    /** Training loss */
    trainLoss: number;
    /** Validation loss */
    valLoss: number;
    /** Learning rate */
    learningRate: number;
    /** Gradient norm */
    gradNorm: number;
    /** Steps per second */
    stepsPerSecond: number;
    /** ETA in seconds */
    etaSeconds: number;
}
/**
 * Training data batch
 */
export interface TrainingBatch {
    /** Input embeddings */
    inputs: Embedding[];
    /** Target outputs */
    targets: Embedding[];
    /** Quality scores */
    qualities: number[];
}
/**
 * Checkpoint data
 */
export interface Checkpoint {
    /** Epoch number */
    epoch: number;
    /** Step number */
    step: number;
    /** Training loss at checkpoint */
    loss: number;
    /** Model weights (serialized) */
    weights: string;
    /** Timestamp */
    timestamp: number;
}
/**
 * Learning Rate Scheduler
 */
export declare class LRScheduler {
    private config;
    private initialLR;
    private currentStep;
    private totalSteps;
    constructor(config: Required<TrainingConfig>, totalSteps: number);
    /**
     * Get learning rate for current step
     */
    getLR(): number;
    /**
     * Step the scheduler
     */
    step(): void;
    /**
     * Reset scheduler
     */
    reset(): void;
}
/**
 * Training Metrics Tracker
 */
export declare class MetricsTracker {
    private lossHistory;
    private valLossHistory;
    private gradNormHistory;
    private startTime;
    private stepTimes;
    /**
     * Record training loss
     */
    recordLoss(loss: number): void;
    /**
     * Record validation loss
     */
    recordValLoss(loss: number): void;
    /**
     * Record gradient norm
     */
    recordGradNorm(norm: number): void;
    /**
     * Record step time
     */
    recordStepTime(ms: number): void;
    /**
     * Get average loss over last N steps
     */
    avgLoss(n?: number): number;
    /**
     * Get average validation loss
     */
    avgValLoss(n?: number): number;
    /**
     * Get steps per second
     */
    stepsPerSecond(): number;
    /**
     * Get ETA in seconds
     */
    eta(remainingSteps: number): number;
    /**
     * Get best validation loss
     */
    bestValLoss(): number;
    /**
     * Get total duration
     */
    duration(): number;
    /**
     * Get all loss history
     */
    getLossHistory(): number[];
    /**
     * Get all validation loss history
     */
    getValLossHistory(): number[];
    /**
     * Reset tracker
     */
    reset(): void;
}
/**
 * Training Pipeline
 *
 * Full training infrastructure for SONA models.
 */
export declare class TrainingPipeline {
    private config;
    private adapter;
    private ewcManager;
    private metrics;
    private scheduler;
    private batches;
    private checkpoints;
    private currentEpoch;
    private currentStep;
    private bestValLoss;
    private patienceCounter;
    constructor(config?: TrainingConfig, adapter?: LoraAdapter);
    /**
     * Add training batch
     */
    addBatch(inputs: Embedding[], targets: Embedding[], qualities: number[]): void;
    /**
     * Add training data
     */
    addData(data: Array<{
        input: Embedding;
        target: Embedding;
        quality: number;
    }>): void;
    /**
     * Run training
     */
    train(): TrainingResult;
    /**
     * Single training step
     */
    private trainStep;
    /**
     * Validation pass
     */
    private validate;
    /**
     * Save checkpoint
     */
    private saveCheckpoint;
    /**
     * Load checkpoint
     */
    loadCheckpoint(index: number): boolean;
    /**
     * Get current metrics
     */
    getMetrics(): TrainingMetrics;
    /**
     * Get adapter
     */
    getAdapter(): LoraAdapter;
    /**
     * Get EWC manager
     */
    getEwcManager(): EwcManager;
    /**
     * Get checkpoints
     */
    getCheckpoints(): Checkpoint[];
    /**
     * Reset pipeline
     */
    reset(): void;
    private shuffleBatches;
}
/**
 * Training Factory
 *
 * Create pre-configured training pipelines for common scenarios.
 */
export declare class TrainingFactory {
    /**
     * Create pipeline for quick fine-tuning
     */
    static quickFinetune(): TrainingPipeline;
    /**
     * Create pipeline for deep training
     */
    static deepTraining(): TrainingPipeline;
    /**
     * Create pipeline for continual learning
     */
    static continualLearning(ewcLambda?: number): TrainingPipeline;
    /**
     * Create pipeline for federated aggregation
     */
    static federatedAggregation(): TrainingPipeline;
}
//# sourceMappingURL=training.d.ts.map