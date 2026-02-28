"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.LoraManager = exports.LoraAdapter = void 0;
/**
 * Default LoRA configuration
 */
const DEFAULT_LORA_CONFIG = {
    rank: 8,
    alpha: 16,
    dropout: 0.1,
    targetModules: ['query', 'value'],
};
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
class LoraAdapter {
    constructor(config, inputDim = 256, outputDim = 256) {
        this.trainingState = null;
        this.frozen = false;
        this.config = { ...DEFAULT_LORA_CONFIG, ...config };
        this.inputDim = inputDim;
        this.outputDim = outputDim;
        // Initialize weights
        this.weights = this.initializeWeights();
    }
    /**
     * Forward pass through LoRA adapter
     * OPTIMIZED: Uses Float64Array and loop unrolling
     *
     * output = input + scaling * (input @ A @ B)
     */
    forward(input) {
        const rank = this.config.rank;
        const dim = Math.min(input.length, this.inputDim);
        const scaling = this.weights.scaling;
        // Apply dropout during training (simplified check)
        const applyDropout = this.trainingState !== null && this.config.dropout > 0;
        // input @ A (d -> r) - use typed array for hidden
        const hidden = new Float64Array(rank);
        for (let r = 0; r < rank; r++) {
            let sum = 0;
            const loraACol = this.weights.loraA;
            // Unroll loop for better performance
            let i = 0;
            if (applyDropout) {
                for (; i < dim; i++) {
                    if (Math.random() > this.config.dropout) {
                        sum += input[i] * loraACol[i][r];
                    }
                }
            }
            else {
                for (; i + 3 < dim; i += 4) {
                    sum += input[i] * loraACol[i][r] +
                        input[i + 1] * loraACol[i + 1][r] +
                        input[i + 2] * loraACol[i + 2][r] +
                        input[i + 3] * loraACol[i + 3][r];
                }
                for (; i < dim; i++) {
                    sum += input[i] * loraACol[i][r];
                }
            }
            hidden[r] = sum;
        }
        // hidden @ B (r -> d) + residual
        const output = new Array(this.outputDim);
        const loraB = this.weights.loraB;
        for (let i = 0; i < this.outputDim; i++) {
            let delta = 0;
            for (let r = 0; r < rank; r++) {
                delta += hidden[r] * loraB[r][i];
            }
            // Add scaled delta to input (residual connection)
            output[i] = (input[i] || 0) + scaling * delta;
        }
        return output;
    }
    /**
     * Forward with batch processing
     */
    forwardBatch(inputs) {
        return inputs.map(input => this.forward(input));
    }
    /**
     * Backward pass and weight update
     */
    backward(input, gradOutput, learningRate) {
        if (this.frozen)
            return 0;
        const rank = this.config.rank;
        const dim = Math.min(input.length, this.inputDim);
        // Compute hidden activations (for gradient)
        const hidden = new Array(rank).fill(0);
        for (let r = 0; r < rank; r++) {
            for (let i = 0; i < dim; i++) {
                hidden[r] += input[i] * this.weights.loraA[i][r];
            }
        }
        // Gradient for B: hidden^T @ gradOutput
        const gradB = Array(rank).fill(null).map(() => Array(this.outputDim).fill(0));
        for (let r = 0; r < rank; r++) {
            for (let i = 0; i < this.outputDim; i++) {
                gradB[r][i] = hidden[r] * (gradOutput[i] || 0) * this.weights.scaling;
            }
        }
        // Gradient for hidden: gradOutput @ B^T
        const gradHidden = new Array(rank).fill(0);
        for (let r = 0; r < rank; r++) {
            for (let i = 0; i < this.outputDim; i++) {
                gradHidden[r] += (gradOutput[i] || 0) * this.weights.loraB[r][i] * this.weights.scaling;
            }
        }
        // Gradient for A: input^T @ gradHidden
        const gradA = Array(dim).fill(null).map(() => Array(rank).fill(0));
        for (let i = 0; i < dim; i++) {
            for (let r = 0; r < rank; r++) {
                gradA[i][r] = input[i] * gradHidden[r];
            }
        }
        // Update weights
        let totalGrad = 0;
        for (let i = 0; i < dim; i++) {
            for (let r = 0; r < rank; r++) {
                this.weights.loraA[i][r] -= learningRate * gradA[i][r];
                totalGrad += Math.abs(gradA[i][r]);
            }
        }
        for (let r = 0; r < rank; r++) {
            for (let i = 0; i < this.outputDim; i++) {
                this.weights.loraB[r][i] -= learningRate * gradB[r][i];
                totalGrad += Math.abs(gradB[r][i]);
            }
        }
        // Track training state
        if (this.trainingState) {
            this.trainingState.step++;
            this.trainingState.lossHistory.push(totalGrad);
        }
        return totalGrad;
    }
    /**
     * Start training mode
     */
    startTraining(learningRate = 0.001) {
        this.trainingState = {
            step: 0,
            learningRate,
            gradA: Array(this.inputDim).fill(null).map(() => Array(this.config.rank).fill(0)),
            gradB: Array(this.config.rank).fill(null).map(() => Array(this.outputDim).fill(0)),
            lossHistory: [],
        };
    }
    /**
     * End training mode
     */
    endTraining() {
        const state = this.trainingState;
        this.trainingState = null;
        return state;
    }
    /**
     * Freeze adapter (no more updates)
     */
    freeze() {
        this.frozen = true;
    }
    /**
     * Unfreeze adapter
     */
    unfreeze() {
        this.frozen = false;
    }
    /**
     * Check if frozen
     */
    isFrozen() {
        return this.frozen;
    }
    /**
     * Get adapter config
     */
    getConfig() {
        return { ...this.config };
    }
    /**
     * Get adapter weights
     */
    getWeights() {
        return {
            loraA: this.weights.loraA.map(row => [...row]),
            loraB: this.weights.loraB.map(row => [...row]),
            scaling: this.weights.scaling,
        };
    }
    /**
     * Set adapter weights
     */
    setWeights(weights) {
        this.weights = {
            loraA: weights.loraA.map(row => [...row]),
            loraB: weights.loraB.map(row => [...row]),
            scaling: weights.scaling,
        };
    }
    /**
     * Merge adapter into base weights
     *
     * Returns delta to add to base model weights
     */
    merge() {
        const delta = Array(this.inputDim)
            .fill(null)
            .map(() => Array(this.outputDim).fill(0));
        const rank = this.config.rank;
        for (let i = 0; i < this.inputDim; i++) {
            for (let j = 0; j < this.outputDim; j++) {
                for (let r = 0; r < rank; r++) {
                    delta[i][j] += this.weights.loraA[i][r] * this.weights.loraB[r][j];
                }
                delta[i][j] *= this.weights.scaling;
            }
        }
        return delta;
    }
    /**
     * Get number of trainable parameters
     */
    numParameters() {
        return (this.inputDim * this.config.rank) + (this.config.rank * this.outputDim);
    }
    /**
     * Reset to initial weights
     */
    reset() {
        this.weights = this.initializeWeights();
        this.trainingState = null;
        this.frozen = false;
    }
    /**
     * Clone adapter
     */
    clone() {
        const adapter = new LoraAdapter(this.config, this.inputDim, this.outputDim);
        adapter.setWeights(this.getWeights());
        return adapter;
    }
    /**
     * Serialize to JSON
     */
    toJSON() {
        return JSON.stringify({
            config: this.config,
            inputDim: this.inputDim,
            outputDim: this.outputDim,
            weights: this.weights,
            frozen: this.frozen,
        });
    }
    /**
     * Deserialize from JSON
     */
    static fromJSON(json) {
        const data = JSON.parse(json);
        const adapter = new LoraAdapter(data.config, data.inputDim, data.outputDim);
        adapter.setWeights(data.weights);
        if (data.frozen)
            adapter.freeze();
        return adapter;
    }
    initializeWeights() {
        const rank = this.config.rank;
        // Kaiming initialization for A, zero initialization for B
        const loraA = Array(this.inputDim)
            .fill(null)
            .map(() => Array(rank)
            .fill(0)
            .map(() => (Math.random() - 0.5) * Math.sqrt(2 / this.inputDim)));
        const loraB = Array(rank)
            .fill(null)
            .map(() => Array(this.outputDim).fill(0));
        return {
            loraA,
            loraB,
            scaling: this.config.alpha / this.config.rank,
        };
    }
}
exports.LoraAdapter = LoraAdapter;
/**
 * LoRA Manager for multiple adapters
 *
 * Manages a collection of LoRA adapters for different tasks/domains.
 */
class LoraManager {
    constructor(defaultConfig) {
        this.adapters = new Map();
        this.activeAdapterId = null;
        this.defaultConfig = { ...DEFAULT_LORA_CONFIG, ...defaultConfig };
    }
    /**
     * Register a new adapter
     */
    register(id, adapter) {
        this.adapters.set(id, adapter);
    }
    /**
     * Create and register a new adapter
     */
    create(id, config, inputDim, outputDim) {
        const mergedConfig = { ...this.defaultConfig, ...config };
        const adapter = new LoraAdapter(mergedConfig, inputDim, outputDim);
        this.register(id, adapter);
        return adapter;
    }
    /**
     * Get adapter by ID
     */
    get(id) {
        return this.adapters.get(id);
    }
    /**
     * Remove adapter
     */
    remove(id) {
        if (this.activeAdapterId === id) {
            this.activeAdapterId = null;
        }
        return this.adapters.delete(id);
    }
    /**
     * Activate an adapter
     */
    activate(id) {
        if (this.adapters.has(id)) {
            this.activeAdapterId = id;
            return true;
        }
        return false;
    }
    /**
     * Deactivate current adapter
     */
    deactivate() {
        this.activeAdapterId = null;
    }
    /**
     * Get active adapter
     */
    getActive() {
        return this.activeAdapterId ? this.adapters.get(this.activeAdapterId) || null : null;
    }
    /**
     * Get active adapter ID
     */
    getActiveId() {
        return this.activeAdapterId;
    }
    /**
     * Apply active adapter
     */
    forward(input) {
        const active = this.getActive();
        return active ? active.forward(input) : [...input];
    }
    /**
     * List all adapter IDs
     */
    list() {
        return Array.from(this.adapters.keys());
    }
    /**
     * Get adapter count
     */
    count() {
        return this.adapters.size;
    }
    /**
     * Freeze all adapters
     */
    freezeAll() {
        for (const adapter of this.adapters.values()) {
            adapter.freeze();
        }
    }
    /**
     * Unfreeze all adapters
     */
    unfreezeAll() {
        for (const adapter of this.adapters.values()) {
            adapter.unfreeze();
        }
    }
    /**
     * Merge multiple adapters into one
     */
    mergeAdapters(ids, outputId) {
        const adapters = ids.map(id => this.adapters.get(id)).filter(Boolean);
        if (adapters.length === 0)
            return null;
        // Use first adapter as base
        const merged = adapters[0].clone();
        const weights = merged.getWeights();
        // Average weights from other adapters
        for (let i = 1; i < adapters.length; i++) {
            const otherWeights = adapters[i].getWeights();
            for (let row = 0; row < weights.loraA.length && row < otherWeights.loraA.length; row++) {
                for (let col = 0; col < weights.loraA[row].length && col < otherWeights.loraA[row].length; col++) {
                    weights.loraA[row][col] = (weights.loraA[row][col] + otherWeights.loraA[row][col]) / 2;
                }
            }
            for (let row = 0; row < weights.loraB.length && row < otherWeights.loraB.length; row++) {
                for (let col = 0; col < weights.loraB[row].length && col < otherWeights.loraB[row].length; col++) {
                    weights.loraB[row][col] = (weights.loraB[row][col] + otherWeights.loraB[row][col]) / 2;
                }
            }
        }
        merged.setWeights(weights);
        this.register(outputId, merged);
        return merged;
    }
    /**
     * Get statistics
     */
    stats() {
        let totalParams = 0;
        let frozenCount = 0;
        for (const adapter of this.adapters.values()) {
            totalParams += adapter.numParameters();
            if (adapter.isFrozen())
                frozenCount++;
        }
        return {
            totalAdapters: this.adapters.size,
            activeAdapter: this.activeAdapterId,
            totalParameters: totalParams,
            frozenCount,
        };
    }
    /**
     * Clear all adapters
     */
    clear() {
        this.adapters.clear();
        this.activeAdapterId = null;
    }
}
exports.LoraManager = LoraManager;
//# sourceMappingURL=lora.js.map