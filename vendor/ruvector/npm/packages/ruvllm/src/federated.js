"use strict";
/**
 * Federated Learning for SONA
 *
 * Enable distributed learning across ephemeral agents that share
 * trajectories with a central coordinator.
 *
 * Architecture:
 * ```
 * ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
 * │  Agent A    │     │  Agent B    │     │  Agent C    │
 * │ (ephemeral) │     │ (ephemeral) │     │ (ephemeral) │
 * └──────┬──────┘     └──────┬──────┘     └──────┬──────┘
 *        │                   │                   │
 *        │    export()       │    export()       │    export()
 *        ▼                   ▼                   ▼
 *   ┌────────────────────────────────────────────────┐
 *   │            Federated Coordinator               │
 *   │         (persistent, large capacity)           │
 *   └────────────────────────────────────────────────┘
 * ```
 *
 * @example
 * ```typescript
 * import { EphemeralAgent, FederatedCoordinator } from '@ruvector/ruvllm';
 *
 * // Create coordinator (persistent)
 * const coordinator = new FederatedCoordinator('coord-1', { hiddenDim: 256 });
 *
 * // Create ephemeral agent
 * const agent = new EphemeralAgent('agent-1', { hiddenDim: 256 });
 *
 * // Agent processes tasks
 * agent.processTask([0.1, 0.2, ...], 0.85);
 * agent.processTask([0.3, 0.4, ...], 0.92);
 *
 * // Export and aggregate before agent terminates
 * const exportData = agent.exportState();
 * const result = coordinator.aggregate(exportData);
 *
 * console.log(`Accepted: ${result.trajectoriesAccepted}`);
 * ```
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.FederatedCoordinator = exports.EphemeralAgent = void 0;
const sona_1 = require("./sona");
/**
 * Default federated config
 */
const DEFAULT_FEDERATED_CONFIG = {
    hiddenDim: 256,
    embeddingDim: 256,
    microLoraRank: 2,
    baseLoraRank: 8,
    trajectoryCapacity: 500,
    patternClusters: 25,
    ewcLambda: 2000,
    qualityThreshold: 0.4,
};
/**
 * Ephemeral Agent for federated learning
 *
 * Collects trajectories during its session and exports state before termination.
 *
 * @example
 * ```typescript
 * const agent = new EphemeralAgent('agent-1', { hiddenDim: 256 });
 *
 * // Process tasks during session
 * agent.processTask(embedding1, 0.85);
 * agent.processTaskWithRoute(embedding2, 0.92, 'code-model');
 *
 * // Export before termination
 * const exportData = agent.exportState();
 * ```
 */
class EphemeralAgent {
    constructor(agentId, config) {
        this.trajectories = [];
        this.qualitySamples = [];
        this.loraWeights = [];
        this.agentId = agentId;
        this.config = { ...DEFAULT_FEDERATED_CONFIG, ...config };
        this.startTime = Date.now();
        this.reasoningBank = new sona_1.ReasoningBank(0.7);
        // Initialize micro-LoRA weights
        this.loraWeights = new Array(this.config.hiddenDim * this.config.microLoraRank)
            .fill(0)
            .map(() => (Math.random() - 0.5) * 0.01);
    }
    /**
     * Get agent ID
     */
    getAgentId() {
        return this.agentId;
    }
    /**
     * Process a task and record trajectory
     */
    processTrajectory(embedding, activations, quality, route, context = []) {
        const now = Date.now();
        // Store trajectory for export
        this.trajectories.push({
            embedding: [...embedding],
            quality,
            route,
            context: [...context],
            timestamp: now,
        });
        this.qualitySamples.push(quality);
        // Store in local reasoning bank if high quality
        if (quality >= 0.7) {
            this.reasoningBank.store('query_response', embedding);
        }
        // Update local LoRA weights based on quality
        this.updateLoraWeights(embedding, quality);
    }
    /**
     * Simple process task method
     */
    processTask(embedding, quality) {
        this.processTrajectory(embedding, embedding, quality);
    }
    /**
     * Process task with route information
     */
    processTaskWithRoute(embedding, quality, route) {
        this.processTrajectory(embedding, embedding, quality, route);
    }
    /**
     * Apply micro-LoRA to hidden states
     */
    applyMicroLora(input, output) {
        const rank = this.config.microLoraRank;
        const dim = Math.min(input.length, this.config.hiddenDim);
        // Simple low-rank decomposition: output = input + A @ B @ input
        // A is (dim x rank), B is (rank x dim)
        for (let i = 0; i < dim; i++) {
            let delta = 0;
            for (let r = 0; r < rank; r++) {
                let bSum = 0;
                for (let j = 0; j < dim; j++) {
                    const bIdx = r * dim + j;
                    if (bIdx < this.loraWeights.length) {
                        bSum += this.loraWeights[bIdx] * (input[j] || 0);
                    }
                }
                const aIdx = i * rank + r;
                if (aIdx < this.loraWeights.length) {
                    delta += this.loraWeights[aIdx] * bSum;
                }
            }
            output[i] = (input[i] || 0) + delta * 0.1; // Scale factor
        }
    }
    /**
     * Get number of collected trajectories
     */
    trajectoryCount() {
        return this.trajectories.length;
    }
    /**
     * Get average quality
     */
    avgQuality() {
        if (this.qualitySamples.length === 0)
            return 0;
        return this.qualitySamples.reduce((a, b) => a + b, 0) / this.qualitySamples.length;
    }
    /**
     * Get uptime in seconds
     */
    uptimeSeconds() {
        return Math.floor((Date.now() - this.startTime) / 1000);
    }
    /**
     * Get agent stats
     */
    stats() {
        return {
            totalTrajectories: this.trajectories.length,
            avgQuality: this.avgQuality(),
            patternsLearned: this.reasoningBank.stats().totalPatterns,
        };
    }
    /**
     * Force local learning
     */
    forceLearn() {
        // Prune low-performing patterns
        const pruned = this.reasoningBank.prune(0.3, 3);
        return `Pruned ${pruned} patterns, ${this.reasoningBank.stats().totalPatterns} remaining`;
    }
    /**
     * Get learned patterns
     */
    getPatterns() {
        return this.reasoningBank.getByType('query_response');
    }
    /**
     * Clear trajectories (after export)
     */
    clear() {
        this.trajectories = [];
        this.qualitySamples = [];
    }
    /**
     * Export agent state for federation
     *
     * Call this before terminating the agent.
     */
    exportState() {
        // Force learning before export
        this.forceLearn();
        return {
            agentId: this.agentId,
            trajectories: [...this.trajectories],
            stats: this.stats(),
            sessionDurationMs: Date.now() - this.startTime,
            timestamp: Date.now(),
        };
    }
    /**
     * Serialize to JSON
     */
    toJSON() {
        return JSON.stringify(this.exportState());
    }
    updateLoraWeights(embedding, quality) {
        // Simple gradient update based on quality
        const lr = 0.001 * quality;
        const dim = Math.min(embedding.length, this.config.hiddenDim);
        for (let i = 0; i < Math.min(dim, this.loraWeights.length); i++) {
            const grad = embedding[i % embedding.length] * (quality - 0.5);
            this.loraWeights[i] += lr * grad;
        }
    }
}
exports.EphemeralAgent = EphemeralAgent;
/**
 * Federated Learning Coordinator
 *
 * Aggregates learning from multiple ephemeral agents.
 *
 * @example
 * ```typescript
 * const coordinator = new FederatedCoordinator('coord-1', { hiddenDim: 256 });
 *
 * // Aggregate exports from multiple agents
 * for (const agentExport of agentExports) {
 *   const result = coordinator.aggregate(agentExport);
 *   console.log(`Agent ${result.agentId}: ${result.trajectoriesAccepted} accepted`);
 * }
 *
 * // Get coordinator statistics
 * const stats = coordinator.stats();
 * console.log(`Total patterns: ${stats.patternsLearned}`);
 * ```
 */
class FederatedCoordinator {
    constructor(coordinatorId, config) {
        this.contributions = new Map();
        this.totalTrajectories = 0;
        this.consolidationInterval = 50;
        this.qualitySamples = [];
        this.masterLoraWeights = [];
        this.coordinatorId = coordinatorId;
        this.config = {
            ...DEFAULT_FEDERATED_CONFIG,
            trajectoryCapacity: 50000, // Large capacity for coordinator
            patternClusters: 200,
            baseLoraRank: 16, // Deeper for aggregation
            ...config,
        };
        this.reasoningBank = new sona_1.ReasoningBank(this.config.qualityThreshold);
        // Initialize master LoRA weights
        this.masterLoraWeights = new Array(this.config.hiddenDim * this.config.baseLoraRank)
            .fill(0)
            .map(() => (Math.random() - 0.5) * 0.01);
    }
    /**
     * Get coordinator ID
     */
    getCoordinatorId() {
        return this.coordinatorId;
    }
    /**
     * Set quality threshold for accepting trajectories
     */
    setQualityThreshold(threshold) {
        this.config.qualityThreshold = threshold;
    }
    /**
     * Set consolidation interval
     */
    setConsolidationInterval(interval) {
        this.consolidationInterval = interval;
    }
    /**
     * Aggregate agent export into coordinator
     */
    aggregate(exportData) {
        let accepted = 0;
        let rejected = 0;
        // Replay trajectories into master
        for (const traj of exportData.trajectories) {
            if (traj.quality >= this.config.qualityThreshold) {
                // Store pattern
                const patternType = this.routeToPatternType(traj.route);
                this.reasoningBank.store(patternType, traj.embedding);
                this.qualitySamples.push(traj.quality);
                // Update master LoRA weights
                this.updateMasterLora(traj.embedding, traj.quality);
                accepted++;
            }
            else {
                rejected++;
            }
        }
        this.totalTrajectories += accepted;
        // Record contribution
        this.contributions.set(exportData.agentId, {
            trajectoryCount: exportData.trajectories.length,
            avgQuality: exportData.stats.avgQuality,
            timestamp: Date.now(),
            sessionDurationMs: exportData.sessionDurationMs,
        });
        // Auto-consolidate if needed
        const consolidated = this.shouldConsolidate();
        if (consolidated) {
            this.forceConsolidate();
        }
        return {
            agentId: exportData.agentId,
            trajectoriesAccepted: accepted,
            trajectoriesRejected: rejected,
            consolidated,
            totalAgents: this.contributions.size,
            totalTrajectories: this.totalTrajectories,
        };
    }
    /**
     * Force consolidation (learning)
     */
    forceConsolidate() {
        const pruned = this.reasoningBank.prune(0.3, 5);
        return `Consolidated: pruned ${pruned} patterns, ${this.reasoningBank.stats().totalPatterns} remaining`;
    }
    /**
     * Consolidate learning (alias)
     */
    consolidate() {
        return this.forceConsolidate();
    }
    /**
     * Get initial patterns for new agents (warm start)
     */
    getInitialPatterns(k = 10) {
        const allPatterns = [
            ...this.reasoningBank.getByType('query_response'),
            ...this.reasoningBank.getByType('routing'),
        ];
        // Sort by success rate and return top k
        return allPatterns
            .sort((a, b) => b.successRate - a.successRate)
            .slice(0, k);
    }
    /**
     * Get all learned patterns
     */
    getAllPatterns() {
        return [
            ...this.reasoningBank.getByType('query_response'),
            ...this.reasoningBank.getByType('routing'),
            ...this.reasoningBank.getByType('context_retrieval'),
            ...this.reasoningBank.getByType('correction'),
        ];
    }
    /**
     * Find similar patterns
     */
    findPatterns(query, k) {
        return this.reasoningBank.findSimilar(query, k);
    }
    /**
     * Apply coordinator's LoRA to input
     * OPTIMIZED: Pre-compute hidden layer once, reuse typed arrays
     */
    applyLora(input) {
        const rank = this.config.baseLoraRank;
        const dim = Math.min(input.length, this.config.hiddenDim);
        const weightsLen = this.masterLoraWeights.length;
        // Pre-compute hidden layer (input @ B)
        const hidden = new Float64Array(rank);
        for (let r = 0; r < rank; r++) {
            let sum = 0;
            const baseIdx = r * dim;
            // Unroll the inner loop
            let j = 0;
            for (; j + 3 < dim && baseIdx + j + 3 < weightsLen; j += 4) {
                sum += this.masterLoraWeights[baseIdx + j] * (input[j] || 0) +
                    this.masterLoraWeights[baseIdx + j + 1] * (input[j + 1] || 0) +
                    this.masterLoraWeights[baseIdx + j + 2] * (input[j + 2] || 0) +
                    this.masterLoraWeights[baseIdx + j + 3] * (input[j + 3] || 0);
            }
            for (; j < dim && baseIdx + j < weightsLen; j++) {
                sum += this.masterLoraWeights[baseIdx + j] * (input[j] || 0);
            }
            hidden[r] = sum;
        }
        // Compute output (hidden @ A + input)
        const output = new Array(input.length);
        for (let i = 0; i < input.length; i++) {
            if (i < dim) {
                let delta = 0;
                const baseIdx = i * rank;
                for (let r = 0; r < rank && baseIdx + r < weightsLen; r++) {
                    delta += this.masterLoraWeights[baseIdx + r] * hidden[r];
                }
                output[i] = (input[i] || 0) + delta * 0.1;
            }
            else {
                output[i] = input[i] || 0;
            }
        }
        return output;
    }
    /**
     * Get coordinator statistics
     */
    stats() {
        const avgQuality = this.qualitySamples.length > 0
            ? this.qualitySamples.reduce((a, b) => a + b, 0) / this.qualitySamples.length
            : 0;
        return {
            coordinatorId: this.coordinatorId,
            totalAgents: this.contributions.size,
            totalTrajectories: this.totalTrajectories,
            patternsLearned: this.reasoningBank.stats().totalPatterns,
            avgQuality,
            qualityThreshold: this.config.qualityThreshold,
        };
    }
    /**
     * Get contribution history
     */
    getContributions() {
        return new Map(this.contributions);
    }
    /**
     * Get total agent count
     */
    agentCount() {
        return this.contributions.size;
    }
    /**
     * Get total trajectory count
     */
    getTotalTrajectories() {
        return this.totalTrajectories;
    }
    /**
     * Clear all contributions
     */
    clear() {
        this.contributions.clear();
        this.totalTrajectories = 0;
        this.qualitySamples = [];
    }
    /**
     * Export coordinator state
     */
    toJSON() {
        return JSON.stringify({
            coordinatorId: this.coordinatorId,
            stats: this.stats(),
            contributions: Object.fromEntries(this.contributions),
            patterns: this.getAllPatterns(),
        });
    }
    /**
     * Create agent with coordinator's learned patterns
     */
    createAgent(agentId) {
        const agent = new EphemeralAgent(agentId, {
            hiddenDim: this.config.hiddenDim,
            embeddingDim: this.config.embeddingDim,
            microLoraRank: this.config.microLoraRank,
        });
        // Warm start: process initial patterns as positive examples
        const initialPatterns = this.getInitialPatterns(5);
        for (const pattern of initialPatterns) {
            agent.processTask(pattern.embedding, pattern.successRate);
        }
        return agent;
    }
    shouldConsolidate() {
        return this.contributions.size % this.consolidationInterval === 0 &&
            this.contributions.size > 0;
    }
    routeToPatternType(route) {
        if (!route)
            return 'query_response';
        if (route.includes('code'))
            return 'query_response';
        if (route.includes('route'))
            return 'routing';
        if (route.includes('memory'))
            return 'context_retrieval';
        return 'query_response';
    }
    updateMasterLora(embedding, quality) {
        const lr = 0.0005 * quality; // Slower learning for coordinator
        const dim = Math.min(embedding.length, this.config.hiddenDim);
        for (let i = 0; i < Math.min(dim, this.masterLoraWeights.length); i++) {
            const grad = embedding[i % embedding.length] * (quality - 0.5);
            this.masterLoraWeights[i] += lr * grad;
            // EWC regularization - prevent large weight changes
            const penalty = this.config.ewcLambda * this.masterLoraWeights[i] * 0.0001;
            this.masterLoraWeights[i] -= penalty;
        }
    }
}
exports.FederatedCoordinator = FederatedCoordinator;
//# sourceMappingURL=federated.js.map