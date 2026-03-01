"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.Sona = exports.SonaEngine = void 0;
exports.isSonaAvailable = isSonaAvailable;
// ============================================================================
// Helper Functions
// ============================================================================
/** Convert any array-like to regular Array (SONA expects number[]) */
function toArray(input) {
    if (Array.isArray(input))
        return input;
    return Array.from(input);
}
// ============================================================================
// Lazy Loading
// ============================================================================
let sonaModule = null;
let sonaLoadError = null;
function getSonaModule() {
    if (sonaModule)
        return sonaModule;
    if (sonaLoadError)
        throw sonaLoadError;
    try {
        sonaModule = require('@ruvector/sona');
        return sonaModule;
    }
    catch (e) {
        sonaLoadError = new Error(`@ruvector/sona is not installed. Install it with:\n` +
            `  npm install @ruvector/sona\n\n` +
            `Original error: ${e.message}`);
        throw sonaLoadError;
    }
}
/** Check if sona is available */
function isSonaAvailable() {
    try {
        getSonaModule();
        return true;
    }
    catch {
        return false;
    }
}
// ============================================================================
// SONA Engine Wrapper
// ============================================================================
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
class SonaEngine {
    /**
     * Create a new SONA engine
     * @param hiddenDim Hidden dimension size (e.g., 256, 512, 768)
     */
    constructor(hiddenDim) {
        const mod = getSonaModule();
        this._native = new mod.SonaEngine(hiddenDim);
    }
    /**
     * Create engine with custom configuration
     * @param config SONA configuration options
     */
    static withConfig(config) {
        const mod = getSonaModule();
        const engine = new SonaEngine(config.hiddenDim);
        // Replace native with configured version
        engine._native = mod.SonaEngine.withConfig(config);
        return engine;
    }
    // -------------------------------------------------------------------------
    // Trajectory Recording
    // -------------------------------------------------------------------------
    /**
     * Begin recording a new trajectory
     * @param queryEmbedding Initial query embedding
     * @returns Trajectory ID for subsequent operations
     */
    beginTrajectory(queryEmbedding) {
        return this._native.beginTrajectory(toArray(queryEmbedding));
    }
    /**
     * Add a step to an active trajectory
     * @param trajectoryId Trajectory ID from beginTrajectory
     * @param activations Layer activations
     * @param attentionWeights Attention weights
     * @param reward Reward signal for this step (0.0 - 1.0)
     */
    addStep(trajectoryId, activations, attentionWeights, reward) {
        this._native.addTrajectoryStep(trajectoryId, toArray(activations), toArray(attentionWeights), reward);
    }
    /**
     * Alias for addStep for API compatibility
     */
    addTrajectoryStep(trajectoryId, activations, attentionWeights, reward) {
        this.addStep(trajectoryId, activations, attentionWeights, reward);
    }
    /**
     * Set the model route for a trajectory
     * @param trajectoryId Trajectory ID
     * @param route Model route identifier (e.g., "gpt-4", "claude-3")
     */
    setRoute(trajectoryId, route) {
        this._native.setTrajectoryRoute(trajectoryId, route);
    }
    /**
     * Add context to a trajectory
     * @param trajectoryId Trajectory ID
     * @param contextId Context identifier
     */
    addContext(trajectoryId, contextId) {
        this._native.addTrajectoryContext(trajectoryId, contextId);
    }
    /**
     * Complete a trajectory and submit for learning
     * @param trajectoryId Trajectory ID
     * @param quality Final quality score (0.0 - 1.0)
     */
    endTrajectory(trajectoryId, quality) {
        this._native.endTrajectory(trajectoryId, quality);
    }
    // -------------------------------------------------------------------------
    // LoRA Transformations
    // -------------------------------------------------------------------------
    /**
     * Apply micro-LoRA transformation (ultra-fast, ~0.1ms)
     * @param input Input vector
     * @returns Transformed output vector
     */
    applyMicroLora(input) {
        return this._native.applyMicroLora(toArray(input));
    }
    /**
     * Apply base-LoRA transformation to a specific layer
     * @param layerIdx Layer index
     * @param input Input vector
     * @returns Transformed output vector
     */
    applyBaseLora(layerIdx, input) {
        return this._native.applyBaseLora(layerIdx, toArray(input));
    }
    // -------------------------------------------------------------------------
    // Learning Control
    // -------------------------------------------------------------------------
    /**
     * Run background learning cycle if due
     * Call this periodically (e.g., every few seconds)
     * @returns Status message if learning occurred, null otherwise
     */
    tick() {
        return this._native.tick();
    }
    /**
     * Force immediate background learning cycle
     * @returns Status message with learning results
     */
    forceLearn() {
        return this._native.forceLearn();
    }
    /**
     * Flush pending instant loop updates
     */
    flush() {
        this._native.flush();
    }
    // -------------------------------------------------------------------------
    // Pattern Retrieval
    // -------------------------------------------------------------------------
    /**
     * Find similar learned patterns to a query
     * @param queryEmbedding Query embedding
     * @param k Number of patterns to return
     * @returns Array of similar patterns
     */
    findPatterns(queryEmbedding, k) {
        return this._native.findPatterns(toArray(queryEmbedding), k);
    }
    // -------------------------------------------------------------------------
    // Engine Control
    // -------------------------------------------------------------------------
    /**
     * Get engine statistics
     * @returns Statistics object
     */
    getStats() {
        const statsJson = this._native.getStats();
        return JSON.parse(statsJson);
    }
    /**
     * Enable or disable the engine
     * @param enabled Whether to enable
     */
    setEnabled(enabled) {
        this._native.setEnabled(enabled);
    }
    /**
     * Check if engine is enabled
     */
    isEnabled() {
        return this._native.isEnabled();
    }
}
exports.SonaEngine = SonaEngine;
// ============================================================================
// Convenience Exports
// ============================================================================
/**
 * SONA namespace with all exports
 */
exports.Sona = {
    Engine: SonaEngine,
    isAvailable: isSonaAvailable,
};
exports.default = exports.Sona;
//# sourceMappingURL=sona-wrapper.js.map