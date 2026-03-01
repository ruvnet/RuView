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
import { Embedding, LearnedPattern, FederatedConfig, AgentExportStats, AgentExport, AgentContribution, AggregationResult, CoordinatorStats } from './types';
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
export declare class EphemeralAgent {
    private agentId;
    private config;
    private trajectories;
    private startTime;
    private qualitySamples;
    private reasoningBank;
    private loraWeights;
    constructor(agentId: string, config?: FederatedConfig);
    /**
     * Get agent ID
     */
    getAgentId(): string;
    /**
     * Process a task and record trajectory
     */
    processTrajectory(embedding: Embedding, activations: Embedding, quality: number, route?: string, context?: string[]): void;
    /**
     * Simple process task method
     */
    processTask(embedding: Embedding, quality: number): void;
    /**
     * Process task with route information
     */
    processTaskWithRoute(embedding: Embedding, quality: number, route: string): void;
    /**
     * Apply micro-LoRA to hidden states
     */
    applyMicroLora(input: number[], output: number[]): void;
    /**
     * Get number of collected trajectories
     */
    trajectoryCount(): number;
    /**
     * Get average quality
     */
    avgQuality(): number;
    /**
     * Get uptime in seconds
     */
    uptimeSeconds(): number;
    /**
     * Get agent stats
     */
    stats(): AgentExportStats;
    /**
     * Force local learning
     */
    forceLearn(): string;
    /**
     * Get learned patterns
     */
    getPatterns(): LearnedPattern[];
    /**
     * Clear trajectories (after export)
     */
    clear(): void;
    /**
     * Export agent state for federation
     *
     * Call this before terminating the agent.
     */
    exportState(): AgentExport;
    /**
     * Serialize to JSON
     */
    toJSON(): string;
    private updateLoraWeights;
}
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
export declare class FederatedCoordinator {
    private coordinatorId;
    private config;
    private contributions;
    private totalTrajectories;
    private consolidationInterval;
    private reasoningBank;
    private qualitySamples;
    private masterLoraWeights;
    constructor(coordinatorId: string, config?: FederatedConfig);
    /**
     * Get coordinator ID
     */
    getCoordinatorId(): string;
    /**
     * Set quality threshold for accepting trajectories
     */
    setQualityThreshold(threshold: number): void;
    /**
     * Set consolidation interval
     */
    setConsolidationInterval(interval: number): void;
    /**
     * Aggregate agent export into coordinator
     */
    aggregate(exportData: AgentExport): AggregationResult;
    /**
     * Force consolidation (learning)
     */
    forceConsolidate(): string;
    /**
     * Consolidate learning (alias)
     */
    consolidate(): string;
    /**
     * Get initial patterns for new agents (warm start)
     */
    getInitialPatterns(k?: number): LearnedPattern[];
    /**
     * Get all learned patterns
     */
    getAllPatterns(): LearnedPattern[];
    /**
     * Find similar patterns
     */
    findPatterns(query: Embedding, k: number): LearnedPattern[];
    /**
     * Apply coordinator's LoRA to input
     * OPTIMIZED: Pre-compute hidden layer once, reuse typed arrays
     */
    applyLora(input: number[]): number[];
    /**
     * Get coordinator statistics
     */
    stats(): CoordinatorStats;
    /**
     * Get contribution history
     */
    getContributions(): Map<string, AgentContribution>;
    /**
     * Get total agent count
     */
    agentCount(): number;
    /**
     * Get total trajectory count
     */
    getTotalTrajectories(): number;
    /**
     * Clear all contributions
     */
    clear(): void;
    /**
     * Export coordinator state
     */
    toJSON(): string;
    /**
     * Create agent with coordinator's learned patterns
     */
    createAgent(agentId: string): EphemeralAgent;
    private shouldConsolidate;
    private routeToPatternType;
    private updateMasterLora;
}
//# sourceMappingURL=federated.d.ts.map