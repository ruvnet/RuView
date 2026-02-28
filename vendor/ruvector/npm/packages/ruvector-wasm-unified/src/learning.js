"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.createLearningEngine = createLearningEngine;
exports.createMicroLoraConfig = createMicroLoraConfig;
exports.createBtspConfig = createBtspConfig;
exports.cosineAnnealingLr = cosineAnnealingLr;
exports.warmupLr = warmupLr;
// ============================================================================
// Factory and Utilities
// ============================================================================
/**
 * Create a learning engine instance
 * @param config Optional configuration
 * @returns Initialized learning engine
 */
function createLearningEngine(config) {
    // Default configuration
    const defaultConfig = {
        defaultLearningRate: 0.001,
        batchSize: 32,
        enableGradientCheckpointing: false,
        ...config,
    };
    // Implementation delegated to WASM module
    return {
        microLoraAdapt: (embedding, opType, loraConfig) => {
            // WASM call: ruvector_learning_micro_lora(embedding, opType, config)
            return new Float32Array(embedding.length);
        },
        sonaPreQuery: (dag, contextWindow = 128) => {
            // WASM call: ruvector_learning_sona_pre_query(dag, contextWindow)
            return {
                original: new Float32Array(0),
                enhanced: new Float32Array(0),
                contextVector: new Float32Array(0),
                confidence: 0,
            };
        },
        btspOneShotLearn: (pattern, signal, btspConfig) => {
            // WASM call: ruvector_learning_btsp(pattern, signal, config)
        },
        updateFromTrajectory: (trajectory, algorithm = 'ppo') => {
            // WASM call: ruvector_learning_update_trajectory(trajectory, algorithm)
            return {
                gradient: new Float32Array(0),
                loss: 0,
                entropy: 0,
                klDivergence: 0,
            };
        },
        computeAdvantages: (rewards, values, gamma = 0.99, lambda = 0.95) => {
            // WASM call: ruvector_learning_compute_gae(rewards, values, gamma, lambda)
            return new Float32Array(rewards.length);
        },
        sampleAction: (state, temperature = 1.0) => {
            // WASM call: ruvector_learning_sample_action(state, temperature)
            return { action: 0, logProb: 0 };
        },
        ewcRegularize: (taskId, importance) => {
            // WASM call: ruvector_learning_ewc(taskId, importance)
        },
        progressiveAddColumn: (taskId, hiddenSize = 256) => {
            // WASM call: ruvector_learning_progressive_add(taskId, hiddenSize)
        },
        experienceReplay: (bufferSize = 10000, batchSize = 32) => {
            // WASM call: ruvector_learning_replay(bufferSize, batchSize)
            return {
                states: [],
                actions: [],
                rewards: [],
                nextStates: [],
                dones: [],
            };
        },
        mamlInnerLoop: (supportSet, innerSteps = 5, innerLr = 0.01) => {
            // WASM call: ruvector_learning_maml_inner(supportSet, innerSteps, innerLr)
            return new Float32Array(0);
        },
        reptileUpdate: (taskBatch, epsilon = 0.1) => {
            // WASM call: ruvector_learning_reptile(taskBatch, epsilon)
        },
        getStats: () => ({
            totalSteps: 0,
            totalEpisodes: 0,
            averageReward: 0,
            averageLoss: 0,
            learningRate: defaultConfig.defaultLearningRate,
            memoryUsage: 0,
            patternsLearned: 0,
            adaptationCount: 0,
        }),
        reset: (keepWeights = false) => {
            // WASM call: ruvector_learning_reset(keepWeights)
        },
        saveCheckpoint: async (path) => {
            // WASM call: ruvector_learning_save(path)
        },
        loadCheckpoint: async (path) => {
            // WASM call: ruvector_learning_load(path)
        },
    };
}
/**
 * Create Micro-LoRA configuration
 * @param rank LoRA rank (default: 8)
 * @param alpha LoRA alpha scaling (default: 16)
 * @param targetModules Modules to apply LoRA to
 */
function createMicroLoraConfig(rank = 8, alpha = 16, targetModules = ['attention', 'ffn']) {
    return {
        rank,
        alpha,
        dropout: 0.05,
        targetModules,
    };
}
/**
 * Create BTSP configuration for one-shot learning
 * @param learningRate Learning rate for plasticity
 * @param eligibilityDecay Decay rate for eligibility traces
 * @param rewardWindow Time window for reward integration
 */
function createBtspConfig(learningRate = 0.1, eligibilityDecay = 0.95, rewardWindow = 100) {
    return {
        learningRate,
        eligibilityDecay,
        rewardWindow,
    };
}
/**
 * Compute cosine annealing learning rate
 * @param step Current step
 * @param totalSteps Total training steps
 * @param lrMax Maximum learning rate
 * @param lrMin Minimum learning rate
 */
function cosineAnnealingLr(step, totalSteps, lrMax = 0.001, lrMin = 0.00001) {
    return lrMin + 0.5 * (lrMax - lrMin) * (1 + Math.cos(Math.PI * step / totalSteps));
}
/**
 * Compute warmup learning rate
 * @param step Current step
 * @param warmupSteps Number of warmup steps
 * @param targetLr Target learning rate after warmup
 */
function warmupLr(step, warmupSteps, targetLr = 0.001) {
    if (step < warmupSteps) {
        return targetLr * (step / warmupSteps);
    }
    return targetLr;
}
//# sourceMappingURL=learning.js.map