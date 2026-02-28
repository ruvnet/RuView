/**
 * LoRA (Low-Rank Adaptation) Runtime
 *
 * Efficient parameter-efficient fine-tuning adapters for LLMs.
 * Supports micro-LoRA (fast, small updates) and base-LoRA (deeper adaptation).
 *
 * @example
 * ```typescript
 * import { LoraAdapter, LoraManager } from '@ruvector/ruvllm';
 *
 * // Create adapter
 * const adapter = new LoraAdapter({
 *   rank: 8,
 *   alpha: 16,
 *   dropout: 0.1,
 *   targetModules: ['query', 'value'],
 * });
 *
 * // Apply to hidden states
 * const output = adapter.forward(hiddenStates);
 *
 * // Manage multiple adapters
 * const manager = new LoraManager();
 * manager.register('task-1', adapter);
 * manager.activate('task-1');
 * ```
 */
import { LoRAConfig } from './types';
/**
 * LoRA adapter weights
 */
export interface LoraWeights {
    /** Down projection matrix (d x r) */
    loraA: number[][];
    /** Up projection matrix (r x d) */
    loraB: number[][];
    /** Scaling factor */
    scaling: number;
}
/**
 * LoRA training state
 */
export interface LoraTrainingState {
    /** Current step */
    step: number;
    /** Learning rate */
    learningRate: number;
    /** Accumulated gradients for A */
    gradA: number[][];
    /** Accumulated gradients for B */
    gradB: number[][];
    /** Loss history */
    lossHistory: number[];
}
/**
 * LoRA Adapter
 *
 * Implements low-rank decomposition for parameter-efficient fine-tuning.
 * W' = W + BA where A is (d x r) and B is (r x d), r << d
 *
 * @example
 * ```typescript
 * const adapter = new LoraAdapter({
 *   rank: 8,
 *   alpha: 16,
 *   inputDim: 768,
 *   outputDim: 768,
 * });
 *
 * // Forward pass
 * const output = adapter.forward(input);
 *
 * // Training step
 * adapter.backward(input, gradOutput, 0.001);
 * ```
 */
export declare class LoraAdapter {
    private config;
    private inputDim;
    private outputDim;
    private weights;
    private trainingState;
    private frozen;
    constructor(config?: Partial<LoRAConfig>, inputDim?: number, outputDim?: number);
    /**
     * Forward pass through LoRA adapter
     * OPTIMIZED: Uses Float64Array and loop unrolling
     *
     * output = input + scaling * (input @ A @ B)
     */
    forward(input: number[]): number[];
    /**
     * Forward with batch processing
     */
    forwardBatch(inputs: number[][]): number[][];
    /**
     * Backward pass and weight update
     */
    backward(input: number[], gradOutput: number[], learningRate: number): number;
    /**
     * Start training mode
     */
    startTraining(learningRate?: number): void;
    /**
     * End training mode
     */
    endTraining(): LoraTrainingState | null;
    /**
     * Freeze adapter (no more updates)
     */
    freeze(): void;
    /**
     * Unfreeze adapter
     */
    unfreeze(): void;
    /**
     * Check if frozen
     */
    isFrozen(): boolean;
    /**
     * Get adapter config
     */
    getConfig(): Required<LoRAConfig>;
    /**
     * Get adapter weights
     */
    getWeights(): LoraWeights;
    /**
     * Set adapter weights
     */
    setWeights(weights: LoraWeights): void;
    /**
     * Merge adapter into base weights
     *
     * Returns delta to add to base model weights
     */
    merge(): number[][];
    /**
     * Get number of trainable parameters
     */
    numParameters(): number;
    /**
     * Reset to initial weights
     */
    reset(): void;
    /**
     * Clone adapter
     */
    clone(): LoraAdapter;
    /**
     * Serialize to JSON
     */
    toJSON(): string;
    /**
     * Deserialize from JSON
     */
    static fromJSON(json: string): LoraAdapter;
    private initializeWeights;
}
/**
 * LoRA Manager for multiple adapters
 *
 * Manages a collection of LoRA adapters for different tasks/domains.
 */
export declare class LoraManager {
    private adapters;
    private activeAdapterId;
    private defaultConfig;
    constructor(defaultConfig?: Partial<LoRAConfig>);
    /**
     * Register a new adapter
     */
    register(id: string, adapter: LoraAdapter): void;
    /**
     * Create and register a new adapter
     */
    create(id: string, config?: Partial<LoRAConfig>, inputDim?: number, outputDim?: number): LoraAdapter;
    /**
     * Get adapter by ID
     */
    get(id: string): LoraAdapter | undefined;
    /**
     * Remove adapter
     */
    remove(id: string): boolean;
    /**
     * Activate an adapter
     */
    activate(id: string): boolean;
    /**
     * Deactivate current adapter
     */
    deactivate(): void;
    /**
     * Get active adapter
     */
    getActive(): LoraAdapter | null;
    /**
     * Get active adapter ID
     */
    getActiveId(): string | null;
    /**
     * Apply active adapter
     */
    forward(input: number[]): number[];
    /**
     * List all adapter IDs
     */
    list(): string[];
    /**
     * Get adapter count
     */
    count(): number;
    /**
     * Freeze all adapters
     */
    freezeAll(): void;
    /**
     * Unfreeze all adapters
     */
    unfreezeAll(): void;
    /**
     * Merge multiple adapters into one
     */
    mergeAdapters(ids: string[], outputId: string): LoraAdapter | null;
    /**
     * Get statistics
     */
    stats(): {
        totalAdapters: number;
        activeAdapter: string | null;
        totalParameters: number;
        frozenCount: number;
    };
    /**
     * Clear all adapters
     */
    clear(): void;
}
//# sourceMappingURL=lora.d.ts.map