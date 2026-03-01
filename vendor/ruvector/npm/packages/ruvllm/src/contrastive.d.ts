/**
 * Contrastive Fine-tuning for RuvLTRA Claude Code Router
 *
 * Uses triplet loss to fine-tune embeddings:
 * - Anchor: task description
 * - Positive: correct agent description
 * - Negative: wrong agent description (hard negative)
 *
 * Goal: minimize distance(anchor, positive) and maximize distance(anchor, negative)
 *
 * @example
 * ```typescript
 * import { ContrastiveTrainer, tripletLoss, infoNCELoss } from '@ruvector/ruvllm';
 *
 * const trainer = new ContrastiveTrainer({
 *   epochs: 10,
 *   batchSize: 16,
 *   margin: 0.5,
 * });
 *
 * // Add triplets
 * trainer.addTriplet(anchorEmb, positiveEmb, negativeEmb, true);
 *
 * // Train and export
 * const results = trainer.train();
 * trainer.exportTrainingData('./output');
 * ```
 */
import { Embedding } from './types';
/**
 * Contrastive training configuration
 */
export interface ContrastiveConfig {
    /** Number of training epochs (default: 10) */
    epochs?: number;
    /** Batch size (default: 16) */
    batchSize?: number;
    /** Learning rate (default: 0.0001) */
    learningRate?: number;
    /** Triplet loss margin (default: 0.5) */
    margin?: number;
    /** InfoNCE temperature (default: 0.07) */
    temperature?: number;
    /** Ratio of hard negatives (default: 0.7) */
    hardNegativeRatio?: number;
    /** Output directory for training data */
    outputPath?: string;
}
/**
 * Training triplet
 */
export interface TrainingTriplet {
    /** Anchor embedding (task) */
    anchor: string;
    anchorEmb: Embedding;
    /** Positive example (correct agent) */
    positive: string;
    positiveEmb: Embedding;
    /** Negative example (wrong agent) */
    negative: string;
    negativeEmb: Embedding;
    /** Whether this is a hard negative */
    isHard: boolean;
}
/**
 * Training history entry
 */
export interface TrainingHistoryEntry {
    epoch: number;
    loss: number;
}
/**
 * Contrastive training results
 */
export interface ContrastiveTrainingResult {
    /** Total triplets trained on */
    tripletCount: number;
    /** Final loss value */
    finalLoss: number;
    /** Initial loss value */
    initialLoss: number;
    /** Improvement percentage */
    improvement: number;
    /** Training history */
    history: TrainingHistoryEntry[];
    /** Duration in ms */
    durationMs: number;
}
/**
 * LoRA configuration for fine-tuning
 */
export interface LoRAExportConfig {
    model_type: string;
    base_model: string;
    output_dir: string;
    lora_r: number;
    lora_alpha: number;
    lora_dropout: number;
    target_modules: string[];
    learning_rate: number;
    num_train_epochs: number;
    per_device_train_batch_size: number;
    gradient_accumulation_steps: number;
    warmup_ratio: number;
    loss_type: string;
    margin: number;
    temperature: number;
    train_data: string;
    eval_data: string;
}
/**
 * Compute cosine similarity between two embeddings
 */
export declare function cosineSimilarity(a: Embedding, b: Embedding): number;
/**
 * Compute triplet loss
 * L = max(0, margin + d(anchor, positive) - d(anchor, negative))
 */
export declare function tripletLoss(anchorEmb: Embedding, positiveEmb: Embedding, negativeEmb: Embedding, margin?: number): number;
/**
 * Compute InfoNCE loss (contrastive)
 */
export declare function infoNCELoss(anchorEmb: Embedding, positiveEmb: Embedding, negativeEmbs: Embedding[], temperature?: number): number;
/**
 * Compute gradient for embedding update (simplified)
 */
export declare function computeGradient(anchorEmb: Embedding, positiveEmb: Embedding, negativeEmb: Embedding, lr?: number): Embedding;
/**
 * Contrastive Trainer for RuvLTRA models
 *
 * Implements triplet loss and InfoNCE loss for embedding fine-tuning.
 */
export declare class ContrastiveTrainer {
    private config;
    private triplets;
    private history;
    private agentEmbeddings;
    constructor(config?: ContrastiveConfig);
    /**
     * Add a training triplet
     */
    addTriplet(anchor: string, anchorEmb: Embedding, positive: string, positiveEmb: Embedding, negative: string, negativeEmb: Embedding, isHard?: boolean): void;
    /**
     * Add agent embedding for reference
     */
    addAgentEmbedding(agentName: string, embedding: Embedding): void;
    /**
     * Get all agent embeddings
     */
    getAgentEmbeddings(): Map<string, Embedding>;
    /**
     * Get triplet count
     */
    getTripletCount(): number;
    /**
     * Simulate training (compute losses without actual backprop)
     * In a full implementation, this would use proper gradient descent
     */
    train(): ContrastiveTrainingResult;
    /**
     * Export training data for external fine-tuning tools
     */
    exportTrainingData(outputPath?: string): string;
    /**
     * Generate LoRA adapter configuration
     */
    generateLoRAConfig(outputPath?: string): LoRAExportConfig;
    /**
     * Generate training script for external tools
     */
    generateTrainingScript(outputPath?: string): string;
    /**
     * Get training history
     */
    getHistory(): TrainingHistoryEntry[];
    /**
     * Reset trainer
     */
    reset(): void;
}
/**
 * Agent Training Data Interface
 */
export interface AgentTrainingData {
    description: string;
    keywords: string[];
    examples: string[];
    confusing_with?: string[];
}
/**
 * Training Example Interface
 */
export interface TrainingExample {
    task: string;
    agent: string;
    complexity?: string;
    confusing_with?: string;
}
/**
 * Dataset Statistics
 */
export interface DatasetStats {
    totalExamples: number;
    contrastivePairs: number;
    agentTypes: number;
    agents: string[];
}
/**
 * Agent Training Data for Claude Code Router
 */
export declare const AGENT_TRAINING_DATA: Record<string, AgentTrainingData>;
/**
 * Generate training dataset from agent data
 */
export declare function generateTrainingDataset(): TrainingExample[];
/**
 * Generate contrastive pairs for training
 */
export declare function generateContrastivePairs(): Array<{
    anchor: string;
    positive: string;
    negative: string;
    isHard: boolean;
}>;
/**
 * Get dataset statistics
 */
export declare function getDatasetStats(): DatasetStats;
//# sourceMappingURL=contrastive.d.ts.map