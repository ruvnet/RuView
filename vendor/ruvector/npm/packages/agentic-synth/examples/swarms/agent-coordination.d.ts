/**
 * Multi-Agent System Coordination Examples
 *
 * Demonstrates agent communication patterns, task distribution,
 * consensus building, load balancing, and fault tolerance scenarios
 * for distributed agent systems.
 *
 * Integrates with:
 * - claude-flow: Swarm initialization and orchestration
 * - ruv-swarm: Enhanced coordination patterns
 * - flow-nexus: Cloud-based agent management
 */
/**
 * Generate communication patterns for multi-agent systems
 */
export declare function agentCommunicationPatterns(): Promise<import("../../dist/index.js").GenerationResult<unknown>>;
/**
 * Generate task distribution data for load balancing
 */
export declare function taskDistributionScenarios(): Promise<import("../../dist/index.js").GenerationResult<unknown>>;
/**
 * Generate consensus protocol data for distributed decision making
 */
export declare function consensusBuildingData(): Promise<import("../../dist/index.js").GenerationResult<unknown>>;
/**
 * Generate load balancing metrics and patterns
 */
export declare function loadBalancingPatterns(): Promise<{
    metrics: import("../../dist/index.js").GenerationResult<unknown>;
    agentMetrics: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate fault tolerance and failure recovery scenarios
 */
export declare function faultToleranceScenarios(): Promise<{
    failures: import("../../dist/index.js").GenerationResult<unknown>;
    recoveryActions: import("../../dist/index.js").GenerationResult<unknown>;
}>;
/**
 * Generate hierarchical swarm coordination data
 */
export declare function hierarchicalCoordination(): Promise<{
    topology: import("../../dist/index.js").GenerationResult<unknown>;
    events: import("../../dist/index.js").GenerationResult<unknown>;
}>;
export declare function runAllCoordinationExamples(): Promise<void>;
//# sourceMappingURL=agent-coordination.d.ts.map